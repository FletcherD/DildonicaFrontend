use crate::config::{AppConfig, DildonicaZoneConfig};
use eframe::egui;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

const NUM_ZONES: usize = 8;
const PLOT_DURATION_SECS: f64 = 4.0;

#[derive(Clone, Copy)]
pub struct ProcessedSample {
    pub timestamp: i32,
    pub zone: usize,
    pub value_raw: f64,
    pub value_normalized: f64,
}

#[derive(PartialEq)]
pub enum Tab {
    Plot,
    Config,
    Midi,
}

pub struct PlotApp {
    pub sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
    pub rx: mpsc::Receiver<ProcessedSample>,
    pub time_begin: Instant,
    pub time_delta: Option<i32>,
    pub zone_configs: Arc<Mutex<[DildonicaZoneConfig; NUM_ZONES]>>,
    pub config_tx: Option<mpsc::Sender<[DildonicaZoneConfig; NUM_ZONES]>>,
    pub config_read_tx: Option<mpsc::Sender<()>>,
    pub app_config: Arc<Mutex<AppConfig>>,
    pub selected_tab: Tab,
}

impl PlotApp {
    pub fn new(
        sensor_data: Arc<Mutex<[Vec<[f64; 2]>; NUM_ZONES]>>,
        rx: mpsc::Receiver<ProcessedSample>,
        zone_configs: Arc<Mutex<[DildonicaZoneConfig; NUM_ZONES]>>,
        config_tx: mpsc::Sender<[DildonicaZoneConfig; NUM_ZONES]>,
        config_read_tx: mpsc::Sender<()>,
        app_config: Arc<Mutex<AppConfig>>,
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

    pub fn current_dildonica_time(&self) -> f64 {
        let cur_machine_time = self.time_begin.elapsed().as_millis() as i32;
        (cur_machine_time - self.time_delta.unwrap_or(0)) as f64 / 1000.0
    }

    pub fn process_incoming_samples(&mut self) {
        let cur_machine_time = self.time_begin.elapsed().as_millis() as i32;
        let cur_dildonica_time = self.current_dildonica_time();

        while let Ok(processed_sample) = self.rx.try_recv() {
            let timestamp = processed_sample.timestamp;
            if self.time_delta.is_none() {
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

            while !zone_data.is_empty()
                && zone_data[0][0] < cur_dildonica_time - PLOT_DURATION_SECS
            {
                zone_data.remove(0);
            }
        }
    }
}

impl eframe::App for PlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_incoming_samples();

        // Tab bar
        egui::TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, Tab::Plot, "Plot");
                ui.selectable_value(&mut self.selected_tab, Tab::Config, "Configuration");
                ui.selectable_value(&mut self.selected_tab, Tab::Midi, "MIDI");
            });
        });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| match self.selected_tab {
            Tab::Plot => {
                super::plot::render_plot_tab(self, ui, ctx);
            }
            Tab::Config => {
                super::config_ui::render_config_tab(self, ui, ctx);
            }
            Tab::Midi => {
                super::midi_ui::render_midi_tab(self, ui, ctx);
            }
        });

        ctx.request_repaint();
    }
}