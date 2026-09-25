mod adb;
mod app;
mod device;
mod files;
mod stream;
mod theme;
mod ui;
mod watcher;
mod wifi; 

use app::TransfelApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TransFEL")
            .with_inner_size([1250.0, 800.0])
            .with_min_inner_size([950.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TransFEL",
        options,
        Box::new(|cc| Ok(Box::new(TransfelApp::new(cc)))),
    )
}