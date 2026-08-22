use super::{DagError, Graph, NH, Workflow};
use crate::PluginSet;
use rmp::encode::{RmpWrite, RmpWriteErr, ValueWriteError};
use std::path::Path;

impl Workflow {
    pub fn write_to_msgpack<P: AsRef<Path>>(
        &self,
        path: P,
        plugin_set: &PluginSet,
        w: &mut impl RmpWrite,
    ) -> Result<Self, DagError> {
        todo!()
    }
}

impl Graph {
    const GRAPH_VERSION_CURRENT: u64 = 1;

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
        // Top-level: [version, nodes, links]
        rmp::encode::write_array_len(w, 3)?;
        rmp::encode::write_u64(w, Self::GRAPH_VERSION_CURRENT)?;
        // nodes: [[n_inputs, n_outputs], ...]
        rmp::encode::write_array_len(w, self.nodes.len() as u32)?;
        let mut oh_local: Vec<u32> = vec![0; self.outputs.len()];
        let mut ih_local: Vec<u32> = vec![0; self.inputs.len()];
        for n in 0..self.nodes.len() {
            let nh = NH::from(n);
            let mut n_inputs = 0u32;
            let mut n_outputs = 0u32;
            for (i, input) in self.node_inputs(nh).enumerate() {
                ih_local[input.idx] = i as u32;
                n_inputs += 1;
            }
            for (i, output) in self.node_outputs(nh).enumerate() {
                oh_local[output.idx] = i as u32;
                n_outputs += 1;
            }
            rmp::encode::write_array_len(w, 2)?;
            rmp::encode::write_u32(w, n_inputs)?;
            rmp::encode::write_u32(w, n_outputs)?;
        }
        // links: [[src_node, local_out, dst_node, local_in], ...]
        rmp::encode::write_array_len(w, self.links.len() as u32)?;
        for link in &self.links {
            let src_node = self.outputs[link.start.idx].node.idx as u32;
            let local_out = oh_local[link.start.idx];
            let dst_node = self.inputs[link.end.idx].node.idx as u32;
            let local_in = ih_local[link.end.idx];
            rmp::encode::write_array_len(w, 4)?;
            rmp::encode::write_u32(w, src_node)?;
            rmp::encode::write_u32(w, local_out)?;
            rmp::encode::write_u32(w, dst_node)?;
            rmp::encode::write_u32(w, local_in)?;
        }
        Ok(())
    }
}

impl<E: RmpWriteErr> From<ValueWriteError<E>> for DagError {
    fn from(_: ValueWriteError<E>) -> Self {
        DagError::WriteError
    }
}
