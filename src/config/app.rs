use super::midi::MidiConfig;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub midi: MidiConfig,
    pub plot_raw: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            midi: MidiConfig::default(),
            plot_raw: false,
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
            // Try loading legacy MIDI config file
            if Path::new("dildonica_midi_config.json").exists() {
                println!("Found legacy MIDI config, migrating to new format...");
                let midi_config = MidiConfig::load_from_file_legacy();
                let app_config = AppConfig {
                    midi: midi_config,
                    plot_raw: false,
                };
                let _ = app_config.save_to_file();
                return app_config;
            }
            println!("No app config file found, using defaults");
        }
        Self::default()
    }
}