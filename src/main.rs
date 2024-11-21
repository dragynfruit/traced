mod app;
mod plugins;
mod windows;

use eframe::epaint::Vec2;
use egui::ViewportBuilder;
use env_logger::Builder;
use log::LevelFilter;

fn main() -> Result<(), eframe::Error> {
    Builder::new()
        .filter(None, LevelFilter::Info)
        .filter_module("wgpu_core", LevelFilter::Warn)
        .init();

    log::info!("Starting Visual Trace application");

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size(Vec2::new(1280.0, 720.0))
            .with_min_inner_size(Vec2::new(800.0, 600.0)),
        ..Default::default()
    };

    eframe::run_native(
        "Visual Trace",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc.egui_ctx.clone())))),
    )
}
