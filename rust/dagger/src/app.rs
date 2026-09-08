use crate::render;
use crate::state::EditorState;
use eframe::egui;
use orc_sdk::Workflow;

pub struct DaggerApp {
    state: EditorState,
}

impl DaggerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, workflow: Workflow) -> Self {
        Self {
            state: EditorState::from_workflow(workflow),
        }
    }
}

impl eframe::App for DaggerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        render::draw(ui, &self.state);
    }
}
