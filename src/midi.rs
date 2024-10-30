use std::error::Error;
use std::io::{stdin, stdout, Write};
use midir::{MidiOutput, MidiOutputConnection, MidiOutputPort};

pub fn create_midi_device() -> Result<MidiOutputConnection, Box<dyn Error>> {
    let midi_out = MidiOutput::new("My Virtual MIDI Device")?;

    // Get an output port
    let out_ports = midi_out.ports();
    let out_port: &MidiOutputPort = match out_ports.len() {
        0 => return Err("no output port found".into()),
        1 => {
            println!("Choosing the only available output port: {}", midi_out.port_name(&out_ports[0])?);
            &out_ports[0]
        },
        _ => {
            println!("\nAvailable output ports:");
            for (i, p) in out_ports.iter().enumerate() {
                println!("{}: {}", i, midi_out.port_name(p)?);
            }
            print!("Please select output port: ");
            stdout().flush()?;
            let mut input = String::new();
            stdin().read_line(&mut input)?;
            out_ports.get(input.trim().parse::<usize>()?)
                     .ok_or("invalid output port selected")?
        }
    };

    println!("\nOpening connection");
    let conn_out = midi_out.connect(out_port, "Dildonica MIDI")?;
    println!("Connection open. Listen to your virtual MIDI device.");

    Ok(conn_out)
}

pub fn send_control_change(conn_out: &mut MidiOutputConnection, control_num: u8, control_value: u8) -> Result<(), Box<dyn Error>> {
    const CC_MSG: u8 = 0xB0;

    conn_out.send(&[CC_MSG, control_num, control_value])?;

    Ok(())
}
