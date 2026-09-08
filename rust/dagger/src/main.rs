mod app;
mod canvas;
mod context_menu;
mod exec;
mod inspect;
mod interaction;
mod layout;
mod link_render;
mod node_render;
mod state;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Dagger",
        options,
        Box::new(|cc| Ok(Box::new(app::DaggerApp::new(cc)))),
    )
}
