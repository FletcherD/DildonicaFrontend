# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run Commands

```bash
# Build the project
cargo build

# Run the application with GUI
cargo run

# Run in headless mode (MIDI output only, no GUI)
cargo run -- --headless
# Or with short option
cargo run -- -l

# Build optimized release version
cargo build --release

# Run optimized release version
cargo run --release

# Run optimized release version in headless mode
cargo run --release -- --headless
# Or with short option
cargo run --release -- -l

# Check for errors without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy

# Run tests (if any exist)
cargo test
```

## Project Overview

DildonicaFrontendRs is a Rust frontend application for the Dildonica - a novel musical instrument. The Dildonica contains 8 coils that form resonant circuits, where manipulating the silicone instrument changes the geometry and thus the oscillation periods of these circuits. This application:

- Connects to the Dildonica device via BLE to receive period measurements from the 8 oscillators
- Normalizes the raw sensor data using exponential averaging to create MIDI control values (0-127)
- Creates a virtual MIDI device that sends control change messages, allowing the instrument to control DAWs and music software
- Provides real-time visualization for hardware testing and signal monitoring
- Offers comprehensive configuration management through both GUI and persistent settings

The normalization process ensures MIDI controls read near 0 when the instrument is at rest, and increase when areas are squeezed or stretched, making it behave like traditional MIDI controller knobs.

## Architecture

The application follows a modular architecture with clear separation of concerns:

### Core Modules

1. **`config/` - Configuration Management**
   - `config/app.rs`: Main application configuration including MIDI, plot, and zone mapping settings
   - `config/device.rs`: Device-specific zone configurations and BLE communication
   - `config/midi.rs`: MIDI output methods, musical scales, and MIDI-specific settings
   - `config/zones.rs`: Zone mapping validation and utility functions
   - `config/mod.rs`: Module exports and re-exports

2. **`gui/` - User Interface**
   - `gui/app.rs`: Main PlotApp struct and core GUI framework
   - `gui/plot.rs`: Real-time sensor data plotting and visualization
   - `gui/config_ui.rs`: Device configuration interface and zone mapping controls
   - `gui/midi_ui.rs`: MIDI configuration interface with method selection
   - `gui/mod.rs`: GUI module exports

3. **Core Files**
   - `main.rs`: Application orchestration, BLE communication, and async task coordination
   - `exponential_average.rs`: Exponential moving average calculations for sensor data
   - `midi.rs`: MIDI device creation, message processing, and output handling

### Data Flow

1. **BLE Communication**: Raw oscillator period measurements from 8 coil sensors
2. **Data Processing**: Exponential averaging to establish baseline, normalization to 0-127 range
3. **Zone Mapping**: Dynamic remapping of device zones to output zones based on user configuration
4. **Output**: 
   - Real-time visualization in GUI plot
   - MIDI control change messages to virtual MIDI device
   - Persistent configuration saving

## Configuration System

The application uses a comprehensive configuration system with GUI controls and automatic persistence:

### Application Configuration (`dildonica_config.json`)
- **MIDI Settings**: Output method (Control Change vs Notes), base values, slopes, musical scales
- **Plot Settings**: Raw vs normalized value display toggle
- **Zone Mapping**: Device zone to output zone mapping (configurable via GUI)

### Device Configuration (BLE-stored)
- **Zone Settings**: Enable/disable, MIDI CC assignments, cycle counts, comparator thresholds
- **Hardware Calibration**: Per-zone sensor configuration parameters

### Zone Mapping Configuration
Zone mapping is now configured through the GUI Configuration tab rather than command line arguments:
- **Interactive Controls**: Drag values to map device zones (0-7) to output zones
- **Preset Buttons**: Reset to Default, Reverse Order
- **Real-time Validation**: Visual feedback for valid/invalid mappings
- **Immediate Effect**: Changes apply instantly to plot and MIDI output
- **Persistence**: Settings automatically saved to configuration file

Example mappings:
- Default: Device zone 0→Output 0, Device zone 1→Output 1, etc.
- Reversed: Device zone 7→Output 0, Device zone 6→Output 1, etc.
- Custom: Any valid one-to-one mapping between device and output zones

## User Interface

The application provides a tabbed interface with three main sections:

### 1. Plot Tab
- **Real-time Visualization**: Scrolling time-series plot of all 8 sensor zones
- **Display Mode Toggle**: Switch between raw sensor values and normalized values
- **Auto-scaling**: Automatic bounds adjustment for optimal visibility
- **Color-coded Zones**: Each zone has a distinct color for easy identification

### 2. Configuration Tab
- **Device Zone Configuration**: Per-zone settings for hardware parameters
- **Zone Mapping Controls**: Interactive zone mapping with visual feedback
- **Device Communication**: Read/Write configuration to/from BLE device
- **Validation**: Real-time feedback for configuration validity

### 3. MIDI Tab
- **Output Method Selection**: Control Change messages vs Note On/Off messages
- **Control Change Settings**: Base control number, control slope
- **Note Settings**: Base note, threshold, velocity slope, musical scale selection
- **Scale Selection**: Support for multiple musical scales (Chromatic, Major, Minor, etc.)

## Configuration Constants

Important constants defined throughout the codebase:
- `SERVICE_UUID` and `CHARACTERISTIC_UUID`: BLE service identifiers
- `PLOT_DURATION_SECS`: Time window for the scrolling plot (4 seconds)
- `NUM_ZONES`: Number of sensor zones (8)
- `EXPONENTIAL_ALPHA`: Smoothing factor for exponential average (0.001)

## Dependencies

Major dependencies include:
- `btleplug`: For Bluetooth Low Energy communication
- `eframe`/`egui`: For GUI framework and real-time plotting
- `egui_plot`: For time-series visualization
- `midir`: For MIDI output and virtual device creation
- `tokio`: For async runtime and inter-task communication
- `serde`: For configuration serialization/deserialization
- `clap`: For command-line argument parsing
- `thiserror`: For structured error handling

## Development Notes

### Code Organization Principles
- **Modular Design**: Each module has a single, clear responsibility
- **Separation of Concerns**: GUI, configuration, BLE, and MIDI logic are isolated
- **Type Safety**: Extensive use of Rust's type system for error prevention
- **Async Design**: Non-blocking BLE communication and GUI updates

### Configuration Management
- All user settings persist automatically in `dildonica_config.json`
- Legacy configuration migration is supported
- GUI changes take effect immediately without restart
- Configuration validation prevents invalid states

### Testing and Quality
- Use `cargo check` for fast compilation checks
- Use `cargo clippy` for linting and code quality
- Use `cargo fmt` for consistent code formatting
- Configuration validation prevents runtime errors

### Debugging Tips
- Headless mode (`--headless`) for MIDI-only operation
- Console output shows BLE connection status and configuration changes
- Real-time plot helps visualize sensor behavior and mapping effects
- Configuration validation provides immediate feedback for invalid settings