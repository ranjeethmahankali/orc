use crate::state::EditorState;
use eframe::egui;

pub struct DaggerApp {
    state: EditorState,
}

impl DaggerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: EditorState::new(),
        }
    }
}

impl eframe::App for DaggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("Dagger - Node Editor");
    }
}
