#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod export;
mod image_ops;
mod model;
mod settings;

use app::AssetpackBuilderForWonderdraft;

fn main() -> eframe::Result {
    let icon = image::load_from_memory(include_bytes!("../assetpack_builder_for_wonderdraft.png"))
        .expect("embedded application icon must be a valid image")
        .into_rgba8();
    let (icon_width, icon_height) = icon.dimensions();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Assetpack Builder for Wonderdraft")
            .with_icon(egui::IconData {
                rgba: icon.into_raw(),
                width: icon_width,
                height: icon_height,
            })
            .with_inner_size([1450.0, 900.0])
            .with_min_inner_size([1050.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "assetpack-builder-for-wonderdraft",
        options,
        Box::new(|cc| Ok(Box::new(AssetpackBuilderForWonderdraft::new(cc)))),
    )
}
