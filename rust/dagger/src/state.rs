use crate::layout;
use eframe::egui;
use orc_sdk::{NodeProperty, Workflow};

pub struct EditorState {
    pub workflow: Workflow,
    pub node_positions: NodeProperty<[f32; 2]>,
    pub node_sizes: NodeProperty<[f32; 2]>,
    pub layout_converged: bool,
    pub pan_offset: egui::Vec2,
    pub zoom: f32,
}

impl EditorState {
    pub fn from_workflow(mut workflow: Workflow) -> Self {
        let node_positions = workflow.create_node_property([0.0f32, 0.0]);
        let node_sizes = workflow.create_node_property([160.0f32, 60.0]);
        let mut state = Self {
            workflow,
            node_positions,
            node_sizes,
            layout_converged: false,
            pan_offset: egui::Vec2::ZERO,
            zoom: 1.0,
        };
        layout::topological_seed(&mut state);
        state
    }
}
