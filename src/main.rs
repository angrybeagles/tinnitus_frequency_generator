// Hide console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod presets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1020.0, 680.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("Tinnitus Frequency Generator"),
        ..Default::default()
    };

    eframe::run_native(
        "Tinnitus Frequency Generator",
        options,
        Box::new(|cc| Ok(Box::new(app::TinnitusApp::new(cc)))),
    )
}
