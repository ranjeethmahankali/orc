use super::{DagError, Graph, Workflow};
use rmp::encode::RmpWrite;
use std::path::Path;

const WORKFLOW_VERSION_CURRENT: u64 = 1;

impl Workflow {
    pub fn read_from_file<P: AsRef<Path>>(_path: P) -> Result<Self, DagError> {
        todo!()
    }
}

impl Graph {
    pub fn write_to_msgpack(&self, w: &mut impl RmpWrite) -> Result<(), DagError> {
        if self.inputs.iter().any(|i| i.deleted)
            || self.outputs.iter().any(|i| i.deleted)
            || self.links.iter().any(|i| i.deleted)
            || self.nodes.iter().any(|i| i.deleted)
        {
            // We don't want to ever serialize a graph with stale deleted elements. Before saving to
            // a file, the caller must first clean up all the stale elements from the graph by
            // calling garbage collection.
            return Err(DagError::GarbageCollectionRequired);
        }
        todo!();
    }
}
