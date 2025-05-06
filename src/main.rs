mod midi;
mod exponential_average;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend, Corner, PlotBounds};
use futures::stream::StreamExt;
use thiserror::Error;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use eframe::egui::Vec2b;
use tokio::sync::mpsc;
use uuid::Uuid;
use std::time::Instant;
use midir::MidiOutputConnection;
use num_traits::abs;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x64696c640000100080000000cafebabe);
const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69630000100080000000cafebabe);
const PLOT_DURATION_SECS: f64 = 4.0;
const NUM_ZONES: usize = 8;
const EXPONENTIAL_ALPHA: f64 = 0.000;

const ZONE_MAP: [usize; NUM_ZONES] = [0,1,2,3,4,5,6,7];
const MIDI_CONTROL_SLOPE: f64 = 20.0;

const MIDI_CONTROL_NUMBER: u8 = 41;

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
    zone: usize,
    value: Option<u32>,
}

impl Sample {
    fn from_bytes(data: &[u8]) -> Result<Self, SampleError> {
        if data.len() < 9 {
            return Err(SampleError::DataTooShort);
        }

        let timestamp = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let value = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let zone = u8::from_le_bytes(data[8..9].try_into().unwrap());

        if zone >= NUM_ZONES as u8 {
            return Err(SampleError::InvalidZone);
        }

        Ok(Sample {
            timestamp,
            value: if value == 0 { None } else { Some(value) },
            zone: zone as usize,
        })
    }
}

#[derive(Clone, Copy)]
struct SampleNormalized {
    timestamp: u32,
    zone: usize,
    value_normalized: f64,
}

fn get_normalized_sample(sample: Sample, zone_averages: &mut [exponential_average::ExponentialAverage; NUM_ZONES]) -> SampleNormalized {
    let zone = ZONE_MAP[sample.zone];
    let value_normalized: f64;
    if let Some(value) = sample.value {
        zone_averages[zone].update(value as f64);
        let average = zone_averages[zone].get_average().unwrap_or(0.0);
        value_normalized = (value as f64 - average) / average;
        // value_normalized = value as f64;
    } else {
        value_normalized = 0.0;
    }
    SampleNormalized {
        zone,
        timestamp: sample.timestamp,
        value_normalized
    }
}

fn send_midi_control_change(midi_device: &mut MidiOutputConnection, sample_normalized: SampleNormalized) {
    let midi_control_value = f64::min(abs(sample_normalized.value_normalized) * MIDI_CONTROL_SLOPE, 1.0);
    let midi_control_value = f64::round(127.0 * midi_control_value) as u8;
    let midi_control_channel = sample_normalized.zone as u8 + MIDI_CONTROL_NUMBER;
    let _ = midi::send_control_change(midi_device, midi_control_channel, midi_control_value);
}

struct PlotApp {
    sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
    rx: mpsc::Receiver<SampleNormalized>,
    time_begin: Instant,
    time_delta: Option<u32>,
}

impl PlotApp {
    fn new(
        sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
        rx: mpsc::Receiver<SampleNormalized>,
    ) -> Self {
        Self {
            sensor_data,
            rx,
            time_begin: Instant::now(),
            time_delta: None
        }
    }
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let cur_machine_time = self.time_begin.elapsed().as_millis() as u32;
        let cur_dildonica_time = (cur_machine_time - self.time_delta.unwrap_or(0)) as f64 / 1000.0;

        while let Ok(sample_normalized) = self.rx.try_recv() {
            let timestamp = sample_normalized.timestamp;
            if let None = self.time_delta {
                self.time_delta = Some(cur_machine_time - timestamp);
            }
            let mut sensor_data = self.sensor_data.lock().unwrap();

            let midi_value = sample_normalized.value_normalized * MIDI_CONTROL_SLOPE * 127.0;
            let zone_data = &mut sensor_data[sample_normalized.zone];
            zone_data.push([timestamp as f64 / 1000.0, midi_value]);

            while zone_data[0][0] < cur_dildonica_time as f64 - PLOT_DURATION_SECS {
                zone_data.remove(0);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let sensor_data = self.sensor_data.lock().unwrap();

            Plot::new("sensor_plot")
                .legend(Legend::default().position(Corner::LeftTop))
                .allow_scroll(false)
                .x_axis_label("Time (seconds)")
                .show(ui, |plot_ui| {
                    for (zone, points) in sensor_data.iter().enumerate() {
                        let plot_points = PlotPoints::new(points.clone());
                        plot_ui.line(Line::new(plot_points).name(format!("Zone {}", zone)));
                        let mut plot_bounds = plot_ui.plot_bounds();
                        plot_bounds.set_x(&PlotBounds::from_min_max(
                            [cur_dildonica_time as f64 - PLOT_DURATION_SECS, 0.0], [cur_dildonica_time as f64, 0.0]));
                        plot_ui.set_plot_bounds(plot_bounds);
                        plot_ui.set_auto_bounds(Vec2b::new(false, true));
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
    let mut zone_averages = [exponential_average::ExponentialAverage::new(EXPONENTIAL_ALPHA); NUM_ZONES];
    let mut midi_device = midi::create_midi_device().unwrap();

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
                        let sample_normalized = get_normalized_sample(sample, &mut zone_averages);
                        send_midi_control_change(&mut midi_device, sample_normalized);
                        tx.send(sample_normalized).await.unwrap();
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
        Box::new(|_cc| Ok(Box::new(PlotApp::new(sensor_data, rx)))),
    )
    .unwrap();

    Ok(())
}

