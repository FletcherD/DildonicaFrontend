use super::app::PlotApp;
use eframe::egui;

pub fn render_config_tab(app: &mut PlotApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut configs = app.zone_configs.lock().unwrap();
        let mut config_changed = false;

        // Zone Mapping Configuration
        ui.heading("Zone Mapping");
        ui.label("Map device zones to output zones (changes how data appears in plot and MIDI output):");

        ui.group(|ui| {
            let mut app_config = app.app_config.lock().unwrap();
            let mut zone_map_changed = false;

            ui.horizontal_wrapped(|ui| {
                for (output_zone, device_zone) in app_config.zone_map.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("Out {}:", output_zone));
                        zone_map_changed |= ui
                            .add(egui::DragValue::new(device_zone).range(0..=7))
                            .on_hover_text(format!("Device zone that maps to output zone {}", output_zone))
                            .changed();
                    });
                }
            });

            ui.horizontal(|ui| {
                if ui.button("Reset to Default").clicked() {
                    app_config.zone_map = (0..8).collect();
                    zone_map_changed = true;
                }

                if ui.button("Reverse Order").clicked() {
                    app_config.zone_map = app_config.zone_map.clone().into_iter().rev().collect();
                    zone_map_changed = true;
                }
            });

            // Validation
            let mut used_zones = [false; 8];
            let mut has_duplicates = false;
            for &zone in &app_config.zone_map {
                if zone < 8 {
                    if used_zones[zone] {
                        has_duplicates = true;
                        break;
                    }
                    used_zones[zone] = true;
                }
            }

            if has_duplicates {
                ui.colored_label(egui::Color32::RED, "⚠ Warning: Duplicate zones detected!");
            } else if app_config.zone_map.len() == 8 && used_zones.iter().all(|&x| x) {
                ui.colored_label(egui::Color32::GREEN, "✓ Valid zone mapping");
            }

            if zone_map_changed {
                if let Err(e) = app_config.save_to_file() {
                    eprintln!("Failed to save app config: {}", e);
                }
                ctx.request_repaint();
            }
        });

        // Application Settings
        ui.separator();
        ui.heading("Application Settings");
        ui.group(|ui| {
            let mut app_config = app.app_config.lock().unwrap();
            let mut app_settings_changed = false;

            ui.horizontal(|ui| {
                ui.label("Exponential Alpha:");
                app_settings_changed |= ui
                    .add(egui::DragValue::new(&mut app_config.exponential_alpha)
                        .range(0.0001..=1.0)
                        .speed(0.0001)
                        .fixed_decimals(4))
                    .on_hover_text("Smoothing factor for exponential averaging (lower = more smoothing)")
                    .changed();
            });

            ui.horizontal(|ui| {
                ui.label("Plot Duration (seconds):");
                app_settings_changed |= ui
                    .add(egui::DragValue::new(&mut app_config.plot_duration_secs)
                        .range(1.0..=30.0)
                        .speed(0.1)
                        .fixed_decimals(1))
                    .on_hover_text("Time window shown in the plot")
                    .changed();
            });

            if app_settings_changed {
                if let Err(e) = app_config.save_to_file() {
                    eprintln!("Failed to save app config: {}", e);
                }
                ctx.request_repaint();
            }
        });

        ui.separator();
        ui.heading("Device Configuration");
        for (zone, config) in configs.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.label(format!("Zone {}", zone));

                config_changed |= ui.checkbox(&mut config.enabled, "Enabled").changed();

                // ui.horizontal(|ui| {
                //     ui.label("MIDI CC:");
                //     config_changed |= ui
                //         .add(egui::Slider::new(&mut config.midi_control, 0..=127))
                //         .changed();
                // });

                ui.horizontal(|ui| {
                    ui.label("Cycle Count Begin:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.cycle_count_begin)
                                .range(0..=100000),
                        )
                        .changed();
                    ui.label("End:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.cycle_count_end)
                                .range(0..=100000),
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Comparator Threshold Low:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.comp_thresh_lo)
                                .range(0..=10000),
                        )
                        .changed();
                    ui.label("High:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.comp_thresh_hi)
                                .range(0..=10000),
                        )
                        .changed();
                });
            });
        }

        ui.horizontal(|ui| {
            if ui.button("Read Config from Device").clicked() {
                if let Some(ref tx) = app.config_read_tx {
                    let _ = tx.try_send(());
                }
            }

            if ui.button("Write Config to Device").clicked() {
                if let Some(ref tx) = app.config_tx {
                    let _ = tx.try_send(*configs);
                }
            }
        });

        if config_changed {
            ctx.request_repaint();
        }
    });
}