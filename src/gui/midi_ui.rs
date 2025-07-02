use super::app::PlotApp;
use crate::config::{MidiOutputMethod, MusicalScale};
use eframe::egui;

pub fn render_midi_tab(app: &mut PlotApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading("MIDI Configuration");

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut app_config = app.app_config.lock().unwrap();
        let mut config_changed = false;

        ui.group(|ui| {
            ui.label("Output Method");
            ui.horizontal(|ui| {
                config_changed |= ui
                    .radio_value(
                        &mut app_config.midi.method,
                        MidiOutputMethod::ControlChange,
                        "Control Change Messages",
                    )
                    .changed();
                config_changed |= ui
                    .radio_value(
                        &mut app_config.midi.method,
                        MidiOutputMethod::Notes,
                        "Note On/Off Messages",
                    )
                    .changed();
            });
        });

        ui.separator();

        match app_config.midi.method {
            MidiOutputMethod::ControlChange => {
                render_control_change_settings(&mut app_config, ui, &mut config_changed);
            }
            MidiOutputMethod::Notes => {
                render_note_settings(&mut app_config, ui, &mut config_changed);
            }
        }

        // Save config if any changes were made
        if config_changed {
            if let Err(e) = app_config.save_to_file() {
                eprintln!("Failed to save app config: {}", e);
            }
            ctx.request_repaint();
        }
    });
}

fn render_control_change_settings(
    app_config: &mut crate::config::AppConfig,
    ui: &mut egui::Ui,
    config_changed: &mut bool,
) {
    ui.group(|ui| {
        ui.label("Control Change Settings");

        ui.horizontal(|ui| {
            ui.label("Base Control Number:");
            *config_changed |= ui
                .add(egui::Slider::new(
                    &mut app_config.midi.control_change_config.base_control_number,
                    0..=127,
                ))
                .changed();
        });

        ui.horizontal(|ui| {
            ui.label("Control Slope:");
            *config_changed |= ui
                .add(
                    egui::DragValue::new(&mut app_config.midi.control_change_config.control_slope)
                        .range(0.1..=100.0)
                        .speed(0.1),
                )
                .changed();
        });

        ui.label("Control Change mode sends MIDI CC messages for each zone.");
        ui.label("Zone 0 uses base control number, zone 1 uses base+1, etc.");
    });
}

fn render_note_settings(
    app_config: &mut crate::config::AppConfig,
    ui: &mut egui::Ui,
    config_changed: &mut bool,
) {
    ui.group(|ui| {
        ui.label("Note Settings");

        ui.horizontal(|ui| {
            ui.label("Base Note:");
            *config_changed |= ui
                .add(egui::Slider::new(
                    &mut app_config.midi.note_config.base_note,
                    0..=127,
                ))
                .changed();
            ui.label(format!(
                "(MIDI note {})",
                app_config.midi.note_config.base_note
            ));
        });

        ui.horizontal(|ui| {
            ui.label("Threshold:");
            *config_changed |= ui
                .add(
                    egui::DragValue::new(&mut app_config.midi.note_config.threshold)
                        .range(0.001..=1.0)
                        .speed(0.001),
                )
                .changed();
        });

        ui.horizontal(|ui| {
            ui.label("Velocity Slope:");
            *config_changed |= ui
                .add(
                    egui::DragValue::new(&mut app_config.midi.note_config.velocity_slope)
                        .range(1.0..=5000.0)
                        .speed(1.0),
                )
                .changed();
        });

        ui.horizontal(|ui| {
            ui.label("Musical Scale:");
            *config_changed |= egui::ComboBox::from_label("")
                .selected_text(app_config.midi.note_config.scale.name())
                .show_ui(ui, |ui| {
                    let mut scale_changed = false;
                    for scale in MusicalScale::all_scales() {
                        scale_changed |= ui
                            .selectable_value(
                                &mut app_config.midi.note_config.scale,
                                *scale,
                                scale.name(),
                            )
                            .changed();
                    }
                    scale_changed
                })
                .inner
                .unwrap_or(false);
        });

        ui.label("Note mode sends Note On when magnitude > threshold,");
        ui.label("Key Pressure while note is on, and Note Off when magnitude < threshold.");
        ui.label("Zones are mapped to notes according to the selected musical scale.");
    });
}