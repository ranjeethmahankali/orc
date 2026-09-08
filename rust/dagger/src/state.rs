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
        let mut node_positions = workflow.create_node_property([0.0f32, 0.0]);
        let node_sizes = workflow.create_node_property([160.0f32, 60.0]);
        // Seed initial positions so nodes aren't all stacked at (0,0).
        {
            let mut pos = node_positions.try_borrow_mut().unwrap();
            let mut i = 0usize;
            for nh in workflow.node_iter() {
                let col = i % 4;
                let row = i / 4;
                pos[nh] = [50.0 + col as f32 * 220.0, 50.0 + row as f32 * 120.0];
                i += 1;
            }
        }
        Self {
            workflow,
            node_positions,
            node_sizes,
            layout_converged: true,
            pan_offset: egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}
