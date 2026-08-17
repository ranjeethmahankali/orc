use super::{DagError, Workflow};
use std::path::Path;

impl Workflow {
    pub fn parse_from_orc_file<P: AsRef<Path>>(_path: P) -> Result<Self, DagError> {
        todo!()
    }
}
