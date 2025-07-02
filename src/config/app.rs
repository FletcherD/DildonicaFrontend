use super::midi::MidiConfig;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub midi: MidiConfig,
    pub plot_raw: bool,
    pub zone_map: Vec<usize>,
    pub exponential_alpha: f64,
    pub plot_duration_secs: f64,
}
fn create_default_zone_map(num_zones: usize) -> Vec<usize> {
    (0..num_zones).collect()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            midi: MidiConfig::default(),
            plot_raw: false,
            zone_map: create_default_zone_map(8), // Default to 8 zones
            exponential_alpha: 0.001,
            plot_duration_secs: 4.0,
        }
    }
}

impl AppConfig {
    const CONFIG_FILE_NAME: &'static str = "dildonica_config.json";

    pub fn save_to_file(&self) -> Result<(), Box<dyn Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(Self::CONFIG_FILE_NAME, json)?;
        println!("App config saved to {}", Self::CONFIG_FILE_NAME);
        Ok(())
    }

    pub fn load_from_file() -> Self {
        if Path::new(Self::CONFIG_FILE_NAME).exists() {
            match fs::read_to_string(Self::CONFIG_FILE_NAME) {
                Ok(json) => match serde_json::from_str(&json) {
                    Ok(config) => {
                        println!("App config loaded from {}", Self::CONFIG_FILE_NAME);
                        return config;
                    }
                    Err(e) => eprintln!("Failed to parse app config file: {}", e),
                },
                Err(e) => eprintln!("Failed to read app config file: {}", e),
            }
        } else {
            println!("No app config file found, using defaults");
        }
        Self::default()
    }
}