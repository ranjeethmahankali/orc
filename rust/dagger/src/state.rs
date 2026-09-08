use orc_sdk::Workflow;

pub struct EditorState {
    pub workflow: Workflow,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            workflow: Workflow::default(),
        }
    }
}
