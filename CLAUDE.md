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

# Plot raw sensor values instead of normalized values (for testing)
cargo run -- --raw
# Or with short option  
cargo run -- -r

# Remap device zones to different output zones/channels
cargo run -- --map 5,6,7,2,1,3,4,0
# Or with short option
cargo run -- -m 5,6,7,2,1,3,4,0

# Combine options (headless + raw + custom mapping)
cargo run -- --headless --raw --map 0,1,2,3,4,5,6,7

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

The normalization process ensures MIDI controls read near 0 when the instrument is at rest, and increase when areas are squeezed or stretched, making it behave like traditional MIDI controller knobs.

### Core Components

1. **BLE Communication**: Connects to a specific BLE device with MAC address "DB:96:90:70:68:A4" and subscribes to notifications from a characteristic.

2. **Data Processing**: 
   - Raw oscillator period measurements are received from the 8 coil-based sensors
   - Exponential averaging tracks the baseline for each sensor when at rest
   - Normalized values (0-127) are calculated as distance from the exponential average

3. **Visualization**:
   - Real-time plotting of normalized sensor data using eframe/egui
   - Displays all 8 sensor zones for hardware testing and monitoring
   - Auto-scrolling time-series plot for observing signal behavior

4. **MIDI Output**:
   - Creates a virtual MIDI device on the host system
   - Sends normalized sensor values as MIDI control change messages (0-127)
   - Functions like traditional MIDI controller knobs for use in DAWs
   - Supports both standard MIDI output and MPE (MIDI Polyphonic Expression)

## Architecture

The application is structured into several modules:

- `main.rs`: Contains the main application logic, BLE connection handling, and GUI implementation
- `exponential_average.rs`: Implements exponential moving average calculations for sensor data
- `midi.rs`: Handles basic MIDI output functionality
- `midi_mpe.rs`: Implements MPE (MIDI Polyphonic Expression) functionality for enhanced control

The main dataflow:
1. BLE notifications -> Raw oscillator period measurements from 8 coil sensors
2. Exponential averaging to establish baseline for each sensor
3. Normalization: calculate distance from baseline, scale to 0-127 range
4. Normalized data sent to GUI for real-time plotting
5. Normalized data sent as MIDI control change messages to virtual MIDI device

## Configuration Constants

Several important configuration constants in `main.rs`:
- `SERVICE_UUID` and `CHARACTERISTIC_UUID`: BLE service identifiers
- `PLOT_DURATION_SECS`: Time window for the scrolling plot (4 seconds)
- `NUM_ZONES`: Number of sensor zones (8)
- `ZONE_MAP`: Default zone mapping array (can be overridden with --map)
- `EXPONENTIAL_ALPHA`: Smoothing factor for exponential average
- `MIDI_CONTROL_SLOPE`: Scaling factor for MIDI control values

## Zone Mapping

The `--map` option allows remapping device zones to different output channels. For example, `--map 5,6,7,2,1,3,4,0` means:
- Device zone 5 → Output zone 0
- Device zone 6 → Output zone 1  
- Device zone 7 → Output zone 2
- Device zone 2 → Output zone 3
- Device zone 1 → Output zone 4
- Device zone 3 → Output zone 5
- Device zone 4 → Output zone 6
- Device zone 0 → Output zone 7

This affects both the plot display and MIDI control change messages, allowing you to customize the layout to match your physical instrument configuration.

## Dependencies

Major dependencies include:
- `btleplug`: For Bluetooth Low Energy communication
- `eframe`/`egui`: For GUI and plotting
- `midir`: For MIDI output
- `tokio`: For async runtime and communication