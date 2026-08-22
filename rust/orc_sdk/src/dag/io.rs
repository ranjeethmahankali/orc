use super::{DagError, Graph, Workflow};
use std::path::Path;

const WORKFLOW_VERSION_CURRENT: u64 = 1;

impl Workflow {
    pub fn read_from_file<P: AsRef<Path>>(_path: P) -> Result<Self, DagError> {
        todo!()
    }
}

impl Graph {
    pub fn write_to_msgpack() {}
}
