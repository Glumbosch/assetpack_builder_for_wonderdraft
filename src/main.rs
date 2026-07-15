#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod export;
mod image_ops;
mod model;

use app::WonderdraftAssetStudio;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wonderdraft Asset Studio")
            .with_inner_size([1450.0, 900.0])
            .with_min_inner_size([1050.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "wonderdraft-asset-studio",
        options,
        Box::new(|cc| Ok(Box::new(WonderdraftAssetStudio::new(cc)))),
    )
}
