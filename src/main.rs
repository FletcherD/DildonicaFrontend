use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints, Legend};
use futures::stream::StreamExt;
use thiserror::Error;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;
use std::collections::BTreeMap;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x64696c640000100080000000cafebabe);
const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69630000100080000000cafebabe);
const MAX_POINTS: usize = 1000;

#[derive(Error, Debug)]
enum SampleError {
    #[error("Data too short")]
    DataTooShort,
    #[error("BLE error: {0}")]
    BleError(#[from] btleplug::Error),
}

#[derive(Clone, Copy)]
struct Sample {
    timestamp: u32,
    value: u32,
    zone: u32,
}

impl Sample {
    fn from_bytes(data: &[u8]) -> Result<Self, SampleError> {
        if data.len() < 6 {
            return Err(SampleError::DataTooShort);
        }

        let timestamp = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let value = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let zone = u32::from_le_bytes(data[8..12].try_into().unwrap());

        Ok(Sample {
            timestamp,
            value,
            zone,
        })
    }
}


struct PlotApp {
    sensor_data: Arc<Mutex<Vec<Sample>>>,
    rx: mpsc::Receiver<Sample>,
}

impl PlotApp {
    fn new(sensor_data: Arc<Mutex<Vec<Sample>>>, rx: mpsc::Receiver<Sample>) -> Self {
        Self {
            sensor_data,
            rx,
        }
    }
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for new data
        while let Ok(data) = self.rx.try_recv() {
            let mut sensor_data = self.sensor_data.lock().unwrap();
            sensor_data.push(data);
            if sensor_data.len() > MAX_POINTS {
                sensor_data.remove(0);
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let data = self.sensor_data.lock().unwrap();

            Plot::new("sensor_plot")
                .legend(Legend::default())
                .show(ui, |plot_ui| {
                    let mut series_data: BTreeMap<u32, Vec<[f64; 2]>> = BTreeMap::new();

                    for sample in data.iter() {
                        series_data.entry(sample.zone)
                            .or_insert_with(Vec::new)
                            .push([sample.timestamp as f64, sample.value as f64]);
                    }

                    for (zone, points) in series_data.iter() {
                        let plot_points: PlotPoints = points.iter().cloned().collect();
                        plot_ui.line(Line::new(plot_points).name(format!("Zone {}", zone)));
                    }
                });
        });

        ctx.request_repaint();
    }
}



#[tokio::main]
async fn main() -> Result<(), SampleError> {
    let sensor_data = Arc::new(Mutex::new(Vec::new()));
    let (tx, mut rx) = mpsc::channel(100);

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
                    Ok(sensor_data) => {
                        tx.send(sensor_data).await.unwrap();
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
        "BLE Sensor Data Plot",
        options,
        Box::new(|_cc| Ok(Box::new(PlotApp::new(sensor_data, rx)))),
    )
    .unwrap();

    Ok(())
}

