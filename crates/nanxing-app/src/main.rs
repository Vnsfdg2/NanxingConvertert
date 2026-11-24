#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod settings;
mod theme;
mod viewport;

use app::NanxingStudioApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Nanxing 6S — Konwerter STEP → SCX")
            .with_inner_size([1440.0, 860.0])
            .with_min_inner_size([1180.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Nanxing 6S — Konwerter STEP → SCX",
        native_options,
        Box::new(|cc| Ok(Box::new(NanxingStudioApp::new(cc)))),
    )
}
