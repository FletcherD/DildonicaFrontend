use std::collections::HashMap;

// MIDI status constants
const STATUS_CONTROL_CHANGE: u8 = 0xB0;
const STATUS_NOTE_ON: u8 = 0x90;
const STATUS_NOTE_OFF: u8 = 0x80;
const STATUS_CHANNEL_AFTERTOUCH: u8 = 0xD0;

// MIDI control change constants
const CHANNEL_DATA_ENTRY_MSB: u8 = 0x06;
const CHANNEL_RPN_LSB: u8 = 0x64;
const CHANNEL_RPN_MSB: u8 = 0x65;

#[derive(Debug)]
struct ZoneConfig {
    master_channel: u8,
    member_channels: Vec<u8>,
    active: bool,
}

pub struct MPEKeyboard {
    // MIDI interface will be added later
    lower_zone: ZoneConfig,
    upper_zone: ZoneConfig,
    active_notes: HashMap<u8, u8>,  // note_number -> channel
    channel_notes: HashMap<u8, u8>, // channel -> note_number
    next_channel_index: usize,
    master_pitch_bend_range: u8,
    note_pitch_bend_range: u8,
    rpn_msb: u8,
    rpn_lsb: u8,
}

impl MPEKeyboard {
    pub fn new() -> Self {
        let mut keyboard = MPEKeyboard {
            lower_zone: ZoneConfig {
                master_channel: 1,
                member_channels: (2..16).collect(), // Default to using all available channels
                active: true,
            },
            upper_zone: ZoneConfig {
                master_channel: 16,
                member_channels: Vec::new(),
                active: false,
            },
            active_notes: HashMap::new(),
            channel_notes: HashMap::new(),
            next_channel_index: 0,
            master_pitch_bend_range: 2,
            note_pitch_bend_range: 48,
            rpn_msb: 0,
            rpn_lsb: 0,
        };

        keyboard.send_mpe_configuration();
        keyboard
    }

    // This will be implemented when MIDI interface is added
    fn send_midi_message(&self, status: u8, data1: u8, data2: Option<u8>) {
        // Placeholder for actual MIDI sending implementation
        let message = match data2 {
            Some(d2) => format!("MIDI Message: [{:02X}, {:02X}, {:02X}]", status, data1, d2),
            None => format!("MIDI Message: [{:02X}, {:02X}]", status, data1),
        };
        println!("{}", message);
    }

    pub fn receive_midi_message(&mut self, message: &[u8]) {
        let message_str = message.iter()
            .map(|byte| format!("{:02X}", byte))
            .collect::<Vec<String>>()
            .join(", ");
        println!("Received MIDI Message: [{}]", message_str);

        let status = message[0];
        let data1 = message[1];
        let data2 = message.get(2).copied();

        let message_type = status & 0xF0;
        let channel = status & 0x0F;

        if message_type == STATUS_CONTROL_CHANGE {
            match data1 {
                CHANNEL_RPN_LSB => self.rpn_lsb = data2.unwrap_or(0),
                CHANNEL_RPN_MSB => self.rpn_msb = data2.unwrap_or(0),
                CHANNEL_DATA_ENTRY_MSB => self.handle_rpn(channel, self.rpn_msb, self.rpn_lsb, data2.unwrap_or(0)),
                _ => (),
            }
        }
    }

    fn send_mpe_configuration(&self) {
        // Select RPN 6 (MPE Configuration)
        self.send_midi_message(STATUS_CONTROL_CHANGE, CHANNEL_RPN_LSB, Some(0x06));
        self.send_midi_message(STATUS_CONTROL_CHANGE, CHANNEL_RPN_MSB, Some(0x00));
        // Set number of member channels (14 for lower zone)
        self.send_midi_message(STATUS_CONTROL_CHANGE, CHANNEL_DATA_ENTRY_MSB, Some(0x0E));

        // Set default pitch bend ranges
        self.send_pitch_bend_range(self.lower_zone.master_channel, self.master_pitch_bend_range);
        for &channel in &self.lower_zone.member_channels {
            self.send_pitch_bend_range(channel, self.note_pitch_bend_range);
        }
    }

    fn send_pitch_bend_range(&self, channel: u8, range_semitones: u8) {
        self.send_midi_message(STATUS_CONTROL_CHANGE | channel, CHANNEL_RPN_LSB, Some(0x00));
        self.send_midi_message(STATUS_CONTROL_CHANGE | channel, CHANNEL_RPN_MSB, Some(0x00));
        self.send_midi_message(STATUS_CONTROL_CHANGE | channel, CHANNEL_DATA_ENTRY_MSB, Some(range_semitones));
    }

    fn get_next_channel(&mut self) -> u8 {
        let available_channels = &self.lower_zone.member_channels;
        let channel = available_channels[self.next_channel_index];
        self.next_channel_index = (self.next_channel_index + 1) % available_channels.len();
        channel
    }

    pub fn handle_key_press(&mut self, note_number: u8, velocity: u8, initial_pressure: u8) {
        let channel = self.get_next_channel();
        self.active_notes.insert(note_number, channel);
        self.channel_notes.insert(channel, note_number);

        // Send Note On with velocity
        self.send_midi_message(STATUS_NOTE_ON | channel, note_number, Some(velocity));

        // Send initial pressure if greater than 0
        if initial_pressure > 0 {
            self.send_midi_message(STATUS_CHANNEL_AFTERTOUCH | channel, initial_pressure, None);
        }
    }

    pub fn handle_key_release(&mut self, note_number: u8, release_velocity: u8) {
        if let Some(&channel) = self.active_notes.get(&note_number) {
            // Send Note Off (using note-on with velocity 0)
            self.send_midi_message(STATUS_NOTE_ON | channel, note_number, Some(0));
            // Clean up tracking
            self.active_notes.remove(&note_number);
            self.channel_notes.remove(&channel);
        }
    }

    pub fn handle_key_pressure_change(&mut self, note_number: u8, new_pressure: u8) {
        if let Some(&channel) = self.active_notes.get(&note_number) {
            // Send Channel Pressure message
            self.send_midi_message(STATUS_CHANNEL_AFTERTOUCH | channel, new_pressure, None);
        }
    }

    fn handle_rpn(&mut self, channel: u8, msb: u8, lsb: u8, value: u8) {
        // This method can be expanded to handle different RPN messages
        // Currently just a placeholder
        println!("Handling RPN - Channel: {}, MSB: {}, LSB: {}, Value: {}",
                channel, msb, lsb, value);
    }
}