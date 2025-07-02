mod exponential_average;
mod midi;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use clap::Parser;
use eframe::egui;
use eframe::egui::Vec2b;
use egui_plot::{Corner, Legend, Line, Plot, PlotBounds, PlotPoints};
use futures::stream::StreamExt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

const SERVICE_UUID: Uuid = Uuid::from_u128(0x64696c640000100080000000cafebabe);
const CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69630000100080000000cafebabe);
const CONFIG_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6f6e69620000100080000000cafebabe);
const DEVICE_MAC: &str = "DB:96:90:70:68:A4";

const PLOT_DURATION_SECS: f64 = 4.0;

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
    #[error("Invalid zone map: {0}")]
    InvalidZoneMap(String),
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

fn parse_zone_map(map_str: &str) -> Result<[usize; NUM_ZONES], SampleError> {
    let parts: Vec<&str> = map_str.split(',').collect();
    if parts.len() != NUM_ZONES {
        return Err(SampleError::InvalidZoneMap(format!(
            "Expected {} zones, got {}",
            NUM_ZONES,
            parts.len()
        )));
    }

    let mut zone_map = [0; NUM_ZONES];
    let mut used_zones = vec![false; NUM_ZONES];

    for (i, part) in parts.iter().enumerate() {
        let zone: usize = part
            .trim()
            .parse()
            .map_err(|_| SampleError::InvalidZoneMap(format!("Invalid zone number: '{}'", part)))?;

        if zone >= NUM_ZONES {
            return Err(SampleError::InvalidZoneMap(format!(
                "Zone {} is out of range (0-{})",
                zone,
                NUM_ZONES - 1
            )));
        }

        if used_zones[zone] {
            return Err(SampleError::InvalidZoneMap(format!(
                "Zone {} is used multiple times",
                zone
            )));
        }

        zone_map[i] = zone;
        used_zones[zone] = true;
    }

    Ok(zone_map)
}

#[derive(Clone, Copy)]
struct ProcessedSample {
    timestamp: i32,
    zone: usize,
    value_raw: f64,
    value_normalized: f64,
}

#[derive(Clone, Copy, Debug)]
struct DildonicaZoneConfig {
    enabled: bool,
    midi_control: u8,
    cycle_count_begin: u32,
    cycle_count_end: u32,
    comp_thresh_lo: u32,
    comp_thresh_hi: u32,
}

impl Default for DildonicaZoneConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            midi_control: 0,
            cycle_count_begin: 1000,
            cycle_count_end: 10000,
            comp_thresh_lo: 100,
            comp_thresh_hi: 4000,
        }
    }
}

impl DildonicaZoneConfig {
    const SIZE: usize = 20; // 1 + 1 + 2 (padding) + 4 + 4 + 4 + 4 = 20 bytes (4-byte aligned)

    fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.enabled as u8;
        bytes[1] = self.midi_control;
        // bytes[2..4] are padding for 4-byte alignment
        bytes[4..8].copy_from_slice(&self.cycle_count_begin.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.cycle_count_end.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.comp_thresh_lo.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.comp_thresh_hi.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, SampleError> {
        if bytes.len() < Self::SIZE {
            return Err(SampleError::DataTooShort);
        }

        Ok(Self {
            enabled: bytes[0] != 0,
            midi_control: bytes[1],
            // Skip bytes[2..4] (padding)
            cycle_count_begin: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            cycle_count_end: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            comp_thresh_lo: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            comp_thresh_hi: u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        })
    }
}

fn process_sample(
    sample: Sample,
    zone_averages: &mut [exponential_average::ExponentialAverage; NUM_ZONES],
    zone_map: &[usize; NUM_ZONES],
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

async fn read_zone_configs(
    device: &btleplug::platform::Peripheral,
    config_char: &btleplug::api::Characteristic,
) -> Result<[DildonicaZoneConfig; NUM_ZONES], SampleError> {
    let data = device.read(config_char).await?;
    let expected_size = DildonicaZoneConfig::SIZE * NUM_ZONES;

    if data.len() != expected_size {
        return Err(SampleError::DataTooShort);
    }

    let mut configs = [DildonicaZoneConfig::default(); NUM_ZONES];
    for i in 0..NUM_ZONES {
        let start = i * DildonicaZoneConfig::SIZE;
        let end = start + DildonicaZoneConfig::SIZE;
        configs[i] = DildonicaZoneConfig::from_bytes(&data[start..end])?;
    }

    Ok(configs)
}

async fn write_zone_configs(
    device: &btleplug::platform::Peripheral,
    config_char: &btleplug::api::Characteristic,
    configs: &[DildonicaZoneConfig; NUM_ZONES],
) -> Result<(), SampleError> {
    let mut data = Vec::with_capacity(DildonicaZoneConfig::SIZE * NUM_ZONES);
    for config in configs {
        data.extend_from_slice(&config.to_bytes());
    }

    device
        .write(config_char, &data, btleplug::api::WriteType::WithResponse)
        .await?;
    Ok(())
}

#[derive(PartialEq)]
enum Tab {
    Plot,
    Config,
    Midi,
}

struct PlotApp {
    sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
    rx: mpsc::Receiver<ProcessedSample>,
    time_begin: Instant,
    time_delta: Option<i32>,
    zone_configs: Arc<Mutex<[DildonicaZoneConfig; NUM_ZONES]>>,
    config_tx: Option<mpsc::Sender<[DildonicaZoneConfig; NUM_ZONES]>>,
    config_read_tx: Option<mpsc::Sender<()>>,
    app_config: Arc<Mutex<midi::AppConfig>>,
    selected_tab: Tab,
}

impl PlotApp {
    fn new(
        sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
        rx: mpsc::Receiver<ProcessedSample>,
        zone_configs: Arc<Mutex<[DildonicaZoneConfig; NUM_ZONES]>>,
        config_tx: mpsc::Sender<[DildonicaZoneConfig; NUM_ZONES]>,
        config_read_tx: mpsc::Sender<()>,
        app_config: Arc<Mutex<midi::AppConfig>>,
    ) -> Self {
        Self {
            sensor_data,
            rx,
            time_begin: Instant::now(),
            time_delta: None,
            zone_configs,
            config_tx: Some(config_tx),
            config_read_tx: Some(config_read_tx),
            app_config,
            selected_tab: Tab::Plot,
        }
    }
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let cur_machine_time = self.time_begin.elapsed().as_millis() as i32;
        let cur_dildonica_time = (cur_machine_time - self.time_delta.unwrap_or(0)) as f64 / 1000.0;

        while let Ok(processed_sample) = self.rx.try_recv() {
            let timestamp = processed_sample.timestamp;
            if self.time_delta == None {
                self.time_delta = Some(cur_machine_time - timestamp);
            }
            let mut sensor_data = self.sensor_data.lock().unwrap();

            let app_config = self.app_config.lock().unwrap();
            let plot_value = if app_config.plot_raw {
                processed_sample.value_raw
            } else {
                processed_sample.value_normalized
            };
            drop(app_config);

            let zone_data = &mut sensor_data[processed_sample.zone];
            zone_data.push([timestamp as f64 / 1000.0, plot_value]);

            while zone_data.len() != 0
                && zone_data[0][0] < cur_dildonica_time as f64 - PLOT_DURATION_SECS
            {
                zone_data.remove(0);
            }
        }

        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, Tab::Plot, "Plot");
                ui.selectable_value(&mut self.selected_tab, Tab::Config, "Configuration");
                ui.selectable_value(&mut self.selected_tab, Tab::Midi, "MIDI");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.selected_tab {
            Tab::Plot => {
                // Plot configuration controls
                ui.horizontal(|ui| {
                    let mut app_config = self.app_config.lock().unwrap();
                    let config_changed = ui.checkbox(&mut app_config.plot_raw, "Show raw sensor values").changed();
                    if config_changed {
                        if let Err(e) = app_config.save_to_file() {
                            eprintln!("Failed to save app config: {}", e);
                        }
                    }
                    if !app_config.plot_raw {
                        ui.label("(showing normalized values)");
                    }
                });

                ui.separator();

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
                                [cur_dildonica_time as f64 - PLOT_DURATION_SECS, 0.0],
                                [cur_dildonica_time as f64, 0.0],
                            ));
                            plot_ui.set_plot_bounds(plot_bounds);
                            plot_ui.set_auto_bounds(Vec2b::new(false, true));
                        }
                    });
            }
            Tab::Config => {
                ui.heading("Zone Configuration");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut configs = self.zone_configs.lock().unwrap();
                    let mut config_changed = false;

                    for (zone, config) in configs.iter_mut().enumerate() {
                        ui.group(|ui| {
                            ui.label(format!("Zone {}", zone));

                            config_changed |= ui.checkbox(&mut config.enabled, "Enabled").changed();

                            ui.horizontal(|ui| {
                                ui.label("MIDI CC:");
                                config_changed |= ui
                                    .add(egui::Slider::new(&mut config.midi_control, 0..=127))
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Cycle Count Begin:");
                                config_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut config.cycle_count_begin)
                                            .range(0..=100000),
                                    )
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Cycle Count End:");
                                config_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut config.cycle_count_end)
                                            .range(0..=100000),
                                    )
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Comparator Threshold Low:");
                                config_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut config.comp_thresh_lo)
                                            .range(0..=10000),
                                    )
                                    .changed();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Comparator Threshold High:");
                                config_changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut config.comp_thresh_hi)
                                            .range(0..=10000),
                                    )
                                    .changed();
                            });
                        });
                        ui.separator();
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Read Config from Device").clicked() {
                            if let Some(ref tx) = self.config_read_tx {
                                let _ = tx.try_send(());
                            }
                        }

                        if ui.button("Write Config to Device").clicked() {
                            if let Some(ref tx) = self.config_tx {
                                let _ = tx.try_send(*configs);
                            }
                        }
                    });

                    if config_changed {
                        ctx.request_repaint();
                    }
                });
            }
            Tab::Midi => {
                ui.heading("MIDI Configuration");

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut app_config = self.app_config.lock().unwrap();
                    let mut config_changed = false;

                    ui.group(|ui| {
                        ui.label("Output Method");
                        ui.horizontal(|ui| {
                            config_changed |= ui.radio_value(&mut app_config.midi.method, midi::MidiOutputMethod::ControlChange, "Control Change Messages").changed();
                            config_changed |= ui.radio_value(&mut app_config.midi.method, midi::MidiOutputMethod::Notes, "Note On/Off Messages").changed();
                        });
                    });

                    ui.separator();

                    match app_config.midi.method {
                        midi::MidiOutputMethod::ControlChange => {
                            ui.group(|ui| {
                                ui.label("Control Change Settings");

                                ui.horizontal(|ui| {
                                    ui.label("Base Control Number:");
                                    config_changed |= ui.add(egui::Slider::new(&mut app_config.midi.control_change_config.base_control_number, 0..=127)).changed();
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Control Slope:");
                                    config_changed |= ui.add(egui::DragValue::new(&mut app_config.midi.control_change_config.control_slope)
                                        .range(0.1..=100.0)
                                        .speed(0.1)).changed();
                                });

                                ui.label("Control Change mode sends MIDI CC messages for each zone.");
                                ui.label("Zone 0 uses base control number, zone 1 uses base+1, etc.");
                            });
                        }
                        midi::MidiOutputMethod::Notes => {
                            ui.group(|ui| {
                                ui.label("Note Settings");

                                ui.horizontal(|ui| {
                                    ui.label("Base Note:");
                                    config_changed |= ui.add(egui::Slider::new(&mut app_config.midi.note_config.base_note, 0..=127)).changed();
                                    ui.label(format!("(MIDI note {})", app_config.midi.note_config.base_note));
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Threshold:");
                                    config_changed |= ui.add(egui::DragValue::new(&mut app_config.midi.note_config.threshold)
                                        .range(0.001..=1.0)
                                        .speed(0.001)).changed();
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Velocity Slope:");
                                    config_changed |= ui.add(egui::DragValue::new(&mut app_config.midi.note_config.velocity_slope)
                                        .range(1.0..=5000.0)
                                        .speed(1.0)).changed();
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Musical Scale:");
                                    config_changed |= egui::ComboBox::from_label("")
                                        .selected_text(app_config.midi.note_config.scale.name())
                                        .show_ui(ui, |ui| {
                                            let mut scale_changed = false;
                                            for scale in midi::MusicalScale::all_scales() {
                                                scale_changed |= ui.selectable_value(&mut app_config.midi.note_config.scale, *scale, scale.name()).changed();
                                            }
                                            scale_changed
                                        })
                                        .inner
                                        .unwrap_or(false);
                                });

                                ui.label("Note mode sends Note On when magnitude > threshold,");
                                ui.label("Key Pressure while note is on, and Note Off when magnitude < threshold.");
                                ui.label("Zones are mapped to notes according to the selected musical scale.");
                            });
                        }
                    }

                    // Save config if any changes were made
                    if config_changed {
                        if let Err(e) = app_config.save_to_file() {
                            eprintln!("Failed to save app config: {}", e);
                        }
                        ctx.request_repaint();
                    }
                });
            }
        });

        ctx.request_repaint();
    }
}

#[tokio::main]
async fn main() -> Result<(), SampleError> {
    // Parse command line arguments
    let args = Args::parse();

    // Parse zone mapping
    let zone_map = if let Some(map_str) = &args.map {
        parse_zone_map(map_str)?
    } else {
        (0..NUM_ZONES).collect::<Vec<_>>().try_into().unwrap() // Use default mapping
    };

    let sensor_data = Arc::new(Mutex::new(Default::default()));
    let zone_configs = Arc::new(Mutex::new([DildonicaZoneConfig::default(); NUM_ZONES]));
    let app_config = Arc::new(Mutex::new(midi::AppConfig::load_from_file()));
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
        match read_zone_configs(&device, config_char).await {
            Ok(configs) => {
                println!("Read initial configuration from device");
                *zone_configs_clone.lock().unwrap() = configs;
            }
            Err(e) => eprintln!("Failed to read initial configuration: {}", e),
        }

        // Also trigger a read after startup
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match read_zone_configs(&device, config_char).await {
            Ok(configs) => {
                println!("Re-read configuration from device after startup");
                *zone_configs_clone.lock().unwrap() = configs;
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
                        match read_zone_configs(&device, config_char).await {
                            Ok(configs) => {
                                println!("Configuration read successfully");
                                *zone_configs_clone.lock().unwrap() = configs;
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
