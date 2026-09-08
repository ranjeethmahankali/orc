use crate::canvas::Transform;
use crate::layout;
use crate::render;
use orc_sdk::{NodeProperty, Workflow};

pub struct EditorState {
    pub workflow: Workflow,
    pub node_positions: NodeProperty<[f32; 2]>,
    pub node_sizes: NodeProperty<[f32; 2]>,
    pub layout_converged: bool,
    /// Canvas to screen transform, driven by pan/zoom input.
    pub view: Transform,
}

impl EditorState {
    pub fn from_workflow(mut workflow: Workflow) -> Self {
        let node_positions = workflow.create_node_property([0.0f32, 0.0]);
        let mut node_sizes = workflow.create_node_property([160.0f32, 60.0]);
        // Compute actual node sizes from pin counts.
        {
            let mut sz = node_sizes.try_borrow_mut().unwrap();
            for nh in workflow.node_iter() {
                let n_in = workflow.node_inputs(nh).count();
                let n_out = workflow.node_outputs(nh).count();
                sz[nh] = [render::NODE_WIDTH, render::node_height(n_in, n_out)];
            }
        }
        let mut state = Self {
            workflow,
            node_positions,
            node_sizes,
            layout_converged: false,
            view: Transform::default(),
        };
        layout::topological_seed(&mut state);
        state
    }
}
