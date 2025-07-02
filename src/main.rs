mod config;
mod exponential_average;
mod gui;
mod midi;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use clap::Parser;
use config::{
    create_default_zone_map, parse_zone_map, read_zone_configs, write_zone_configs,
    AppConfig, DeviceConfigError, DildonicaZoneConfig,
    ZoneMapError,
};
use gui::{PlotApp, ProcessedSample};
use futures::stream::StreamExt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x64696c640000100080000000cafebabe);
const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69630000100080000000cafebabe);
const CONFIG_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69620000100080000000cafebabe);
const DEVICE_MAC: &str = "DB:96:90:70:68:A4";


const EXPONENTIAL_AVERAGE_ALPHA: f64 = 0.001;

const NUM_ZONES: usize = 8;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about = "Dildonica - BLE sensor to MIDI converter")]
struct Args {
    /// Run in headless mode (no GUI, only MIDI output)
    #[arg(short = 'l', long)]
    headless: bool,
    /// Zone mapping (comma-separated list of 8 zone numbers, e.g., "5,6,7,2,1,3,4,0")
    #[arg(short, long)]
    map: Option<String>,
}

#[derive(Error, Debug)]
enum SampleError {
    #[error("Data too short")]
    DataTooShort,
    #[error("Invalid zone")]
    InvalidZone,
    #[error("BLE error: {0}")]
    BleError(#[from] btleplug::Error),
    #[error("Zone map error: {0}")]
    ZoneMapError(#[from] ZoneMapError),
    #[error("Device config error: {0}")]
    DeviceConfigError(#[from] DeviceConfigError),
}

#[derive(Clone, Copy)]
struct Sample {
    timestamp: i32,
    zone: usize,
    value: Option<i32>,
}

impl Sample {
    fn from_bytes(data: &[u8]) -> Result<Self, SampleError> {
        if data.len() < 9 {
            return Err(SampleError::DataTooShort);
        }

        let timestamp = i32::from_le_bytes(data[0..4].try_into().unwrap());
        let value = i32::from_le_bytes(data[4..8].try_into().unwrap());
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




fn process_sample(
    sample: Sample,
    zone_averages: &mut [exponential_average::ExponentialAverage; NUM_ZONES],
    zone_map: &[usize],
) -> ProcessedSample {
    // Find which output zone this device zone maps to
    let zone = zone_map
        .iter()
        .position(|&x| x == sample.zone)
        .unwrap_or(sample.zone);
    let (value_raw, value_normalized) = if let Some(value) = sample.value {
        let raw = value as f64;
        zone_averages[zone].update(raw);
        let average = zone_averages[zone].get_average().unwrap_or(0.0);
        let normalized = (raw - average) / average;
        (raw, normalized)
    } else {
        (0.0, 0.0)
    };

    ProcessedSample {
        zone,
        timestamp: sample.timestamp,
        value_raw,
        value_normalized,
    }
}




#[tokio::main]
async fn main() -> Result<(), SampleError> {
    // Parse command line arguments
    let args = Args::parse();

    // Parse zone mapping
    let zone_map = if let Some(map_str) = &args.map {
        parse_zone_map(map_str, NUM_ZONES)?
    } else {
        create_default_zone_map(NUM_ZONES)
    };

    let sensor_data = Arc::new(Mutex::new(Default::default()));
    let zone_configs = Arc::new(Mutex::new([DildonicaZoneConfig::default(); NUM_ZONES]));
    let app_config = Arc::new(Mutex::new(AppConfig::load_from_file()));
    let (tx, rx) = mpsc::channel(100);
    let (config_tx, config_rx) = mpsc::channel::<[DildonicaZoneConfig; NUM_ZONES]>(10);
    let (config_read_tx, config_read_rx) = mpsc::channel::<()>(10);
    let mut zone_averages =
        [exponential_average::ExponentialAverage::new(EXPONENTIAL_AVERAGE_ALPHA); NUM_ZONES];
    let mut midi_device = midi::create_midi_device().unwrap();
    let mut midi_processor = midi::MidiProcessor::new();

    // Spawn BLE connection and data processing task
    let zone_map_copy = zone_map;
    let zone_configs_clone = zone_configs.clone();
    let app_config_clone = app_config.clone();
    let ble_handle = tokio::spawn(async move {
        println!("Starting");

        let manager = Manager::new().await.unwrap();
        let adapters = manager.adapters().await.unwrap();
        let central = adapters
            .into_iter()
            .next()
            .expect("No Bluetooth adapters found");

        central.start_scan(ScanFilter::default()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let peripherals = central.peripherals().await.unwrap();
        let device = peripherals
            .into_iter()
            .find(|p| p.address().to_string() == DEVICE_MAC)
            .expect("Device not found");

        println!("Connecting to device...");
        device.connect().await.unwrap();

        println!("Discovering services...");
        device.discover_services().await.unwrap();

        let chars = device.characteristics();
        let sample_char = chars
            .iter()
            .find(|c| c.uuid == Uuid::from_str(&CHARACTERISTIC_UUID.to_string()).unwrap())
            .expect("Sample characteristic not found");

        let config_char = chars
            .iter()
            .find(|c| c.uuid == Uuid::from_str(&CONFIG_CHARACTERISTIC_UUID.to_string()).unwrap())
            .expect("Config characteristic not found");

        // Read initial configuration
        match read_zone_configs(&device, config_char, NUM_ZONES).await {
            Ok(configs) => {
                println!("Read initial configuration from device");
                let configs_array: [DildonicaZoneConfig; NUM_ZONES] = configs.try_into().unwrap();
                *zone_configs_clone.lock().unwrap() = configs_array;
            }
            Err(e) => eprintln!("Failed to read initial configuration: {}", e),
        }

        // Also trigger a read after startup
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match read_zone_configs(&device, config_char, NUM_ZONES).await {
            Ok(configs) => {
                println!("Re-read configuration from device after startup");
                let configs_array: [DildonicaZoneConfig; NUM_ZONES] = configs.try_into().unwrap();
                *zone_configs_clone.lock().unwrap() = configs_array;
            }
            Err(e) => eprintln!("Failed to re-read configuration after startup: {}", e),
        }

        if sample_char.properties.contains(CharPropFlags::NOTIFY) {
            println!("Subscribing to notifications...");
            device.subscribe(&sample_char).await.unwrap();

            let mut notification_stream = device.notifications().await.unwrap();
            println!("Listening for notifications...");

            let mut config_rx = config_rx;
            let mut config_read_rx = config_read_rx;
            loop {
                tokio::select! {
                    Some(data) = notification_stream.next() => {
                        match Sample::from_bytes(&data.value) {
                            Ok(sample) => {
                                let processed_sample = process_sample(sample, &mut zone_averages, &zone_map_copy);
                                {
                                    let app_config = app_config_clone.lock().unwrap();
                                    let _ = midi_processor.process_sample(&mut midi_device, processed_sample.zone, processed_sample.value_normalized, &app_config.midi);
                                }
                                if tx.send(processed_sample).await.is_err() {
                                    println!("Exiting");
                                    break;
                                }
                            }
                            Err(e) => eprintln!("Error parsing sensor data: {}", e),
                        };
                    }
                    Some(new_configs) = config_rx.recv() => {
                        println!("Writing new configuration to device...");
                        match write_zone_configs(&device, config_char, &new_configs).await {
                            Ok(()) => {
                                println!("Configuration written successfully");
                                *zone_configs_clone.lock().unwrap() = new_configs;
                            }
                            Err(e) => eprintln!("Failed to write configuration: {}", e),
                        }
                    }
                    Some(()) = config_read_rx.recv() => {
                        println!("Reading configuration from device...");
                        match read_zone_configs(&device, config_char, NUM_ZONES).await {
                            Ok(configs) => {
                                println!("Configuration read successfully");
                                let configs_array: [DildonicaZoneConfig; NUM_ZONES] = configs.try_into().unwrap();
                                *zone_configs_clone.lock().unwrap() = configs_array;
                            }
                            Err(e) => eprintln!("Failed to read configuration: {}", e),
                        }
                    }
                }
            }
        } else {
            println!("Sample characteristic does not support notifications");
        }
    });

    // Run GUI if not in headless mode
    if !args.headless {
        let options = eframe::NativeOptions::default();
        eframe::run_native(
            "Dildonica Sensor Data Plot",
            options,
            Box::new(move |_cc| {
                Ok(Box::new(PlotApp::new(
                    sensor_data,
                    rx,
                    zone_configs,
                    config_tx,
                    config_read_tx,
                    app_config,
                )))
            }),
        )
        .unwrap();
    } else {
        println!("Running in headless mode (MIDI output only)");
        // Keep the program running in headless mode
        ble_handle.await.unwrap();
    }

    Ok(())
}
