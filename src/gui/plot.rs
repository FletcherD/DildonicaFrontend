use super::app::PlotApp;
use eframe::egui::{self, Vec2b};
use egui_plot::{Corner, Legend, Line, Plot, PlotBounds, PlotPoints};


pub fn render_plot_tab(app: &mut PlotApp, ui: &mut egui::Ui, _ctx: &egui::Context) {
    // Plot configuration controls
    ui.horizontal(|ui| {
        let mut app_config = app.app_config.lock().unwrap();
        let config_changed = ui
            .checkbox(&mut app_config.plot_raw, "Show raw sensor values")
            .changed();
        if config_changed {
            if let Err(e) = app_config.save_to_file() {
                eprintln!("Failed to save app config: {}", e);
            }
        }
        if !app_config.plot_raw {
            ui.label("(showing normalized values)");
        }
    });

    ui.separator();

    let sensor_data = app.sensor_data.lock().unwrap();
    let cur_dildonica_time = app.current_dildonica_time();

    Plot::new("sensor_plot")
        .legend(Legend::default().position(Corner::LeftTop))
        .allow_scroll(false)
        .x_axis_label("Time (seconds)")
        .show(ui, |plot_ui| {
            for (zone, points) in sensor_data.iter().enumerate() {
                let plot_points = PlotPoints::new(points.clone());
                plot_ui.line(Line::new(plot_points).name(format!("Zone {}", zone)));
                let mut plot_bounds = plot_ui.plot_bounds();
                let plot_duration = {
                    let config = app.app_config.lock().unwrap();
                    config.plot_duration_secs
                };
                plot_bounds.set_x(&PlotBounds::from_min_max(
                    [cur_dildonica_time - plot_duration, 0.0],
                    [cur_dildonica_time, 0.0],
                ));
                plot_ui.set_plot_bounds(plot_bounds);
                plot_ui.set_auto_bounds(Vec2b::new(false, true));
            }
        });
}