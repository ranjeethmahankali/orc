use super::{DagError, Workflow};
use std::path::Path;

impl Workflow {
    pub fn parse_from_orc_file<P: AsRef<Path>>(path: P) -> Result<Self, DagError> {
        todo!()
    }
}
