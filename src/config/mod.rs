pub mod app;
pub mod device;
pub mod midi;
pub mod zones;

// Re-export commonly used types for convenience
pub use app::AppConfig;
pub use device::{DeviceConfigError, DildonicaZoneConfig, read_zone_configs, write_zone_configs};
pub use midi::{ControlChangeConfig, MidiConfig, MidiOutputMethod, MusicalScale, NoteConfig};
pub use zones::{create_default_zone_map, parse_zone_map, ZoneMapError};