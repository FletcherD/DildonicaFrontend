mod midi;
mod exponential_average;

use std::cmp::min;
use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, PlotBounds};
use futures::stream::StreamExt;
use thiserror::Error;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;
use std::collections::{BTreeMap};
use std::error::Error;
use midir::MidiOutputConnection;
use num_traits::abs;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x64696c640000100080000000cafebabe);
const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69630000100080000000cafebabe);
const MAX_POINTS: usize = 100;
const NUM_ZONES: usize = 8;
const EXPONENTIAL_ALPHA: f64 = 0.001;

#[derive(Error, Debug)]
enum SampleError {
    #[error("Data too short")]
    DataTooShort,
    #[error("Invalid zone")]
    InvalidZone,
    #[error("BLE error: {0}")]
    BleError(#[from] btleplug::Error),
}

#[derive(Clone, Copy)]
struct Sample {
    timestamp: u32,
    value: Option<u32>,
    zone: u32,
}

impl Sample {
    fn from_bytes(data: &[u8]) -> Result<Self, SampleError> {
        if data.len() < 12 {
            return Err(SampleError::DataTooShort);
        }

        let timestamp = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let value = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let zone = u32::from_le_bytes(data[8..12].try_into().unwrap());

        if zone >= NUM_ZONES as u32 {
            return Err(SampleError::InvalidZone);
        }

        Ok(Sample {
            timestamp,
            value: if value == 0 { None } else { Some(value) },
            zone,
        })
    }
}


struct PlotApp {
    sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
    rx: mpsc::Receiver<Sample>,
    zone_averages: Arc<Mutex<[exponential_average::ExponentialAverage; NUM_ZONES]>>,

    midi_device: MidiOutputConnection
}

impl PlotApp {
    fn new(
        sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
        rx: mpsc::Receiver<Sample>,
        zone_averages: Arc<Mutex<[exponential_average::ExponentialAverage; NUM_ZONES]>>,
    ) -> Self {
        Self {
            sensor_data,
            rx,
            zone_averages,
            midi_device: midi::create_midi_device().unwrap(),
        }
    }
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for new data
        while let Ok(sample) = self.rx.try_recv() {
            let mut sensor_data = self.sensor_data.lock().unwrap();
            let mut zone_averages = self.zone_averages.lock().unwrap();
            let zone_index = sample.zone as usize;

            let normalized_value: f64;
            if let Some(value) = sample.value {
                zone_averages[zone_index].update(value as f64);
                let average = zone_averages[zone_index].get_average().unwrap_or(0.0);
                normalized_value = (value as f64 - average) / average;
                // normalized_value = value as f64;
            } else {
                normalized_value = 0.0;
            }

            let midi_control_value = f64::min(abs(normalized_value) * 20.0, 1.0);
            let midi_control_value = f64::round(127.0 * midi_control_value) as u8;
            let midi_control_channel = sample.zone as u8 + 41;
            let _ = midi::send_control_change(&mut self.midi_device, midi_control_channel, midi_control_value);

            let zone_data = &mut sensor_data[zone_index];
            //zone_data.push([sample.timestamp as f64, sample.value as f64]);
            zone_data.push([sample.timestamp as f64 / 1000.0, normalized_value]);

            if zone_data.len() > MAX_POINTS {
                zone_data.remove(0);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let sensor_data = self.sensor_data.lock().unwrap();

            Plot::new("sensor_plot")
                .legend(Legend::default())
                .allow_scroll(false)
                .x_axis_label("Time (seconds)")
                .show(ui, |plot_ui| {
                    for (zone, points) in sensor_data.iter().enumerate() {
                        let plot_points = PlotPoints::new(points.clone());
                        plot_ui.line(Line::new(plot_points).name(format!("Zone {}", zone)));
                    }
                });
        });

        ctx.request_repaint();
    }
}

#[tokio::main]
async fn main() -> Result<(), SampleError> {
    let sensor_data = Arc::new(Mutex::new(Default::default()));
    let (tx, rx) = mpsc::channel(100);
    let zone_averages = Arc::new(Mutex::new([(); NUM_ZONES].map(|_| exponential_average::ExponentialAverage::new(EXPONENTIAL_ALPHA))));

    tokio::spawn(async move {
        println!("Starting");
        let device_mac = "DB:96:90:70:68:A4";

        let manager = Manager::new().await.unwrap();
        let adapters = manager.adapters().await.unwrap();
        let central = adapters.into_iter().next().expect("No Bluetooth adapters found");

        central.start_scan(ScanFilter::default()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let peripherals = central.peripherals().await.unwrap();
        let device = peripherals
            .into_iter()
            .find(|p| p.address().to_string() == device_mac)
            .expect("Device not found");

        println!("Connecting to device...");
        device.connect().await.unwrap();

        println!("Discovering services...");
        device.discover_services().await.unwrap();

        let chars = device.characteristics();
        let char = chars
            .iter()
            .find(|c| c.uuid == Uuid::from_str(&CHARACTERISTIC_UUID.to_string()).unwrap())
            .expect("Characteristic not found");

        if char.properties.contains(CharPropFlags::NOTIFY) {
            println!("Subscribing to notifications...");
            device.subscribe(&char).await.unwrap();

            let mut notification_stream = device.notifications().await.unwrap();
            println!("Listening for notifications...");

            while let Some(data) = notification_stream.next().await {
                match Sample::from_bytes(&data.value) {
                    Ok(sample) => {
                        tx.send(sample).await.unwrap();
                    }
                    Err(e) => eprintln!("Error parsing sensor data: {}", e)
                };
            }
        } else {
            println!("Characteristic does not support notifications");
        }
    });

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Dildonica Sensor Data Plot",
        options,
        Box::new(|_cc| Ok(Box::new(PlotApp::new(sensor_data, rx, zone_averages)))),
    )
    .unwrap();

    Ok(())
}

