use crate::canvas::Transform;
use crate::layout;
use crate::render;
use eframe::egui;
use orc_sdk::{NodeProperty, Workflow};

pub struct EditorState {
    pub workflow: Workflow,
    pub node_positions: NodeProperty<[f32; 2]>,
    pub node_sizes: NodeProperty<[f32; 2]>,
    /// Set on every node that takes part in a cycle. The graph is allowed to hold cycles, so
    /// these are drawn as an error rather than rejected.
    pub node_in_cycle: NodeProperty<bool>,
    pub layout_converged: bool,
    /// Node sizes are derived from measured label text, which needs a live frame, so they are
    /// computed on the first frame rather than at construction.
    pub needs_measure: bool,
    /// Canvas to screen transform, driven by pan/zoom input.
    pub view: Transform,
}

impl EditorState {
    pub fn from_workflow(mut workflow: Workflow) -> Self {
        let node_positions = workflow.create_node_property([0.0f32, 0.0]);
        let node_sizes =
            workflow.create_node_property([render::MIN_NODE_WIDTH, render::node_height(1, 1)]);
        let node_in_cycle = workflow.create_node_property(false);
        let mut state = Self {
            workflow,
            node_positions,
            node_sizes,
            node_in_cycle,
            layout_converged: false,
            needs_measure: true,
            view: Transform::default(),
        };
        layout::topological_seed(&mut state);
        state
    }

    /// Re-measure node sizes from the current labels, and let the layout settle again since
    /// the simulation reads those sizes.
    pub fn measure(&mut self, ctx: &egui::Context) {
        if let Ok(mut sizes) = self.node_sizes.try_borrow_mut() {
            render::measure_nodes(ctx, &self.workflow, &mut sizes);
        }
        self.needs_measure = false;
        self.layout_converged = false;
    }
}
