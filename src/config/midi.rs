use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MidiOutputMethod {
    ControlChange,
    Notes,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MusicalScale {
    Chromatic,
    Major,
    Minor,
    Pentatonic,
    Blues,
    Dorian,
    Mixolydian,
    Lydian,
    Phrygian,
    Locrian,
    WholeTone,
    Diminished,
}

impl MusicalScale {
    pub fn intervals(&self) -> &'static [u8] {
        match self {
            MusicalScale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            MusicalScale::Major => &[0, 2, 4, 5, 7, 9, 11],
            MusicalScale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            MusicalScale::Pentatonic => &[0, 2, 4, 7, 9],
            MusicalScale::Blues => &[0, 3, 5, 6, 7, 10],
            MusicalScale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            MusicalScale::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            MusicalScale::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            MusicalScale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            MusicalScale::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            MusicalScale::WholeTone => &[0, 2, 4, 6, 8, 10],
            MusicalScale::Diminished => &[0, 2, 3, 5, 6, 8, 9, 11],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            MusicalScale::Chromatic => "Chromatic",
            MusicalScale::Major => "Major",
            MusicalScale::Minor => "Minor",
            MusicalScale::Pentatonic => "Pentatonic",
            MusicalScale::Blues => "Blues",
            MusicalScale::Dorian => "Dorian",
            MusicalScale::Mixolydian => "Mixolydian",
            MusicalScale::Lydian => "Lydian",
            MusicalScale::Phrygian => "Phrygian",
            MusicalScale::Locrian => "Locrian",
            MusicalScale::WholeTone => "Whole Tone",
            MusicalScale::Diminished => "Diminished",
        }
    }

    pub fn all_scales() -> &'static [MusicalScale] {
        &[
            MusicalScale::Chromatic,
            MusicalScale::Major,
            MusicalScale::Minor,
            MusicalScale::Pentatonic,
            MusicalScale::Blues,
            MusicalScale::Dorian,
            MusicalScale::Mixolydian,
            MusicalScale::Lydian,
            MusicalScale::Phrygian,
            MusicalScale::Locrian,
            MusicalScale::WholeTone,
            MusicalScale::Diminished,
        ]
    }

    pub fn map_zone_to_note(&self, base_note: u8, zone: usize) -> u8 {
        let intervals = self.intervals();
        let scale_len = intervals.len();

        if scale_len == 0 {
            return base_note;
        }

        let octave = zone / scale_len;
        let scale_index = zone % scale_len;
        let note_offset = intervals[scale_index] + (octave as u8 * 12);

        (base_note + note_offset).min(127)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiConfig {
    pub method: MidiOutputMethod,
    pub control_change_config: ControlChangeConfig,
    pub note_config: NoteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlChangeConfig {
    pub base_control_number: u8,
    pub control_slope: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteConfig {
    pub base_note: u8,
    pub threshold: f64,
    pub velocity_slope: f64,
    pub scale: MusicalScale,
}

impl Default for MidiConfig {
    fn default() -> Self {
        Self {
            method: MidiOutputMethod::ControlChange,
            control_change_config: ControlChangeConfig {
                base_control_number: 41,
                control_slope: 20.0,
            },
            note_config: NoteConfig {
                base_note: 60, // Middle C
                threshold: 0.1,
                velocity_slope: 100.0,
                scale: MusicalScale::Chromatic,
            },
        }
    }
}

impl MidiConfig {
    pub fn load_from_file_legacy() -> Self {
        const LEGACY_CONFIG_FILE_NAME: &str = "dildonica_midi_config.json";
        if Path::new(LEGACY_CONFIG_FILE_NAME).exists() {
            match fs::read_to_string(LEGACY_CONFIG_FILE_NAME) {
                Ok(json) => match serde_json::from_str(&json) {
                    Ok(config) => {
                        println!("Legacy MIDI config loaded from {}", LEGACY_CONFIG_FILE_NAME);
                        return config;
                    }
                    Err(e) => eprintln!("Failed to parse legacy MIDI config file: {}", e),
                },
                Err(e) => eprintln!("Failed to read legacy MIDI config file: {}", e),
            }
        }
        Self::default()
    }
}