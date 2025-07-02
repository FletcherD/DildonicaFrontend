use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};
use std::error::Error;
use std::io::{stdin, stdout, Write};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiOutputMethod {
    ControlChange,
    Notes,
}

#[derive(Debug, Clone)]
pub struct MidiConfig {
    pub method: MidiOutputMethod,
    pub control_change_config: ControlChangeConfig,
    pub note_config: NoteConfig,
}

#[derive(Debug, Clone)]
pub struct ControlChangeConfig {
    pub base_control_number: u8,
    pub control_slope: f64,
}

#[derive(Debug, Clone)]
pub struct NoteConfig {
    pub base_note: u8,
    pub threshold: f64,
    pub velocity_slope: f64,
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
            },
        }
    }
}

pub struct MidiProcessor {
    note_states: [bool; 8], // Track which notes are currently on
}

impl MidiProcessor {
    pub fn new() -> Self {
        Self {
            note_states: [false; 8],
        }
    }

    pub fn process_sample(
        &mut self,
        conn_out: &mut MidiOutputConnection,
        zone: usize,
        normalized_value: f64,
        config: &MidiConfig,
    ) -> Result<(), Box<dyn Error>> {
        match config.method {
            MidiOutputMethod::ControlChange => self.send_control_change(
                conn_out,
                zone,
                normalized_value,
                &config.control_change_config,
            ),
            MidiOutputMethod::Notes => {
                self.send_note(conn_out, zone, normalized_value, &config.note_config)
            }
        }
    }

    fn send_control_change(
        &self,
        conn_out: &mut MidiOutputConnection,
        zone: usize,
        normalized_value: f64,
        config: &ControlChangeConfig,
    ) -> Result<(), Box<dyn Error>> {
        let midi_control_value = f64::min(normalized_value.abs() * config.control_slope, 1.0);
        let midi_control_value = (127.0 * midi_control_value).round() as u8;
        let midi_control_channel = zone as u8 + config.base_control_number;
        send_control_change(conn_out, midi_control_channel, midi_control_value)
    }

    fn send_note(
        &mut self,
        conn_out: &mut MidiOutputConnection,
        zone: usize,
        normalized_value: f64,
        config: &NoteConfig,
    ) -> Result<(), Box<dyn Error>> {
        if zone >= 8 {
            return Ok(()); // Safety check
        }

        let magnitude = normalized_value.abs();
        let note_number = config.base_note + zone as u8;

        if magnitude > config.threshold {
            // Calculate velocity based on magnitude
            let velocity = f64::min(magnitude * config.velocity_slope, 127.0) as u8;
            let velocity = velocity.max(1); // Ensure velocity is at least 1

            if !self.note_states[zone] {
                // Send note on
                send_note_on(conn_out, note_number, velocity)?;
                self.note_states[zone] = true;
            } else {
                // Send key pressure (aftertouch)
                send_key_pressure(conn_out, note_number, velocity)?;
            }
        } else if self.note_states[zone] {
            // Send note off
            send_note_off(conn_out, note_number)?;
            self.note_states[zone] = false;
        }

        Ok(())
    }
}

pub fn create_midi_device() -> Result<MidiOutputConnection, Box<dyn Error>> {
    let midi_out = MidiOutput::new("My Virtual MIDI Device")?;

    // Get an output port
    let out_ports = midi_out.ports();
    let out_port: &MidiOutputPort = match out_ports.len() {
        0 => return Err("no output port found".into()),
        1 => {
            println!(
                "Choosing the only available output port: {}",
                midi_out.port_name(&out_ports[0])?
            );
            &out_ports[0]
        }
        _ => {
            println!("\nAvailable output ports:");
            for (i, p) in out_ports.iter().enumerate() {
                println!("{}: {}", i, midi_out.port_name(p)?);
            }
            print!("Please select output port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            out_ports
                .get(input.trim().parse::<usize>()?)
                .ok_or("invalid output port selected")?
        }
    };

    println!("\nOpening connection");
    let conn_out = midi_out.connect(out_port, "Dildonica MIDI")?;
    println!("Connection open. Listen to your virtual MIDI device.");

    Ok(conn_out)
}

pub fn send_control_change(
    conn_out: &mut MidiOutputConnection,
    control_num: u8,
    control_value: u8,
) -> Result<(), Box<dyn Error>> {
    const CC_MSG: u8 = 0xB0;
    conn_out.send(&[CC_MSG, control_num, control_value])?;
    Ok(())
}

pub fn send_note_on(
    conn_out: &mut MidiOutputConnection,
    note: u8,
    velocity: u8,
) -> Result<(), Box<dyn Error>> {
    const NOTE_ON_MSG: u8 = 0x90;
    conn_out.send(&[NOTE_ON_MSG, note, velocity])?;
    Ok(())
}

pub fn send_note_off(conn_out: &mut MidiOutputConnection, note: u8) -> Result<(), Box<dyn Error>> {
    const NOTE_OFF_MSG: u8 = 0x80;
    conn_out.send(&[NOTE_OFF_MSG, note, 0])?;
    Ok(())
}

pub fn send_key_pressure(
    conn_out: &mut MidiOutputConnection,
    note: u8,
    pressure: u8,
) -> Result<(), Box<dyn Error>> {
    const KEY_PRESSURE_MSG: u8 = 0xA0;
    conn_out.send(&[KEY_PRESSURE_MSG, note, pressure])?;
    Ok(())
}
