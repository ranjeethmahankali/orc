use super::{DagError, Workflow};
use std::path::Path;

impl Workflow {
    pub fn read_from_file<P: AsRef<Path>>(_path: P) -> Result<Self, DagError> {
        todo!()
    }
}
