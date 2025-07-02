use super::app::PlotApp;
use eframe::egui;

pub fn render_config_tab(app: &mut PlotApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading("Zone Configuration");

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut configs = app.zone_configs.lock().unwrap();
        let mut config_changed = false;

        for (zone, config) in configs.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.label(format!("Zone {}", zone));

                config_changed |= ui.checkbox(&mut config.enabled, "Enabled").changed();

                ui.horizontal(|ui| {
                    ui.label("MIDI CC:");
                    config_changed |= ui
                        .add(egui::Slider::new(&mut config.midi_control, 0..=127))
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Cycle Count Begin:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.cycle_count_begin)
                                .range(0..=100000),
                        )
                        .changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Cycle Count End:");
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
                });

                ui.horizontal(|ui| {
                    ui.label("Comparator Threshold High:");
                    config_changed |= ui
                        .add(
                            egui::DragValue::new(&mut config.comp_thresh_hi)
                                .range(0..=10000),
                        )
                        .changed();
                });
            });
            ui.separator();
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