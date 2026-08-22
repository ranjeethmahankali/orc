use super::{DagError, Graph, NH, NodeInfo, Workflow};
use crate::{OrcHandle, PluginSet, TypeOwner};
use rmp::{
    decode::RmpRead,
    encode::{RmpWrite, RmpWriteErr, ValueWriteError},
};

fn serialize_handle(
    handle: &OrcHandle,
    plugin_set: &PluginSet,
    arena: &crate::ContextArena<Vec<u8>>,
) -> Result<Vec<u8>, DagError> {
    match plugin_set.get_type_owner(handle.type_id) {
        Some(TypeOwner::BuiltIn(_)) => {
            let mut buf = Vec::new();
            crate::try_serialize_handle(handle, &mut buf).map_err(|_| DagError::WriteError)?;
            Ok(buf)
        }
        Some(TypeOwner::Plugin(plugin_idx, _)) => plugin_set.plugins()[*plugin_idx]
            .serialize_deck(arena, handle, |buf| buf.clone())
            .map_err(|_| DagError::WriteError),
        None => Err(DagError::WriteError),
    }
}

impl Workflow {
    const WORKFLOW_MSGPACK_VERSION_CURRENT: u64 = 1;

    pub fn read_from_msgpack(_src: &mut impl RmpRead) -> Result<Self, DagError> {
        todo!()
    }

    pub fn write_to_msgpack(
        &self,
        plugin_set: &PluginSet,
        arena: &crate::ContextArena<Vec<u8>>,
        w: &mut impl RmpWrite,
    ) -> Result<(), DagError> {
        // Top-level workflow map with 10 fields.
        rmp::encode::write_map_len(w, 10)?;
        // "version"
        rmp::encode::write_str(w, "version")?;
        rmp::encode::write_u64(w, Self::WORKFLOW_MSGPACK_VERSION_CURRENT)?;
        // "graph": populate local index maps as a side effect.
        rmp::encode::write_str(w, "graph")?;
        let mut oh_local = Vec::new();
        let mut ih_local = Vec::new();
        self.graph
            .write_to_msgpack(w, &mut oh_local, &mut ih_local)?;
        // "node_infos": array of [tag, payload], one entry per node.
        rmp::encode::write_str(w, "node_infos")?;
        {
            let node_infos = self.node_infos.try_borrow()?;
            rmp::encode::write_array_len(w, node_infos.len() as u32)?;
            for info in node_infos.iter() {
                match info {
                    NodeInfo::Constant(handle) => {
                        let bytes = serialize_handle(handle, plugin_set, arena)?;
                        rmp::encode::write_array_len(w, 2)?;
                        rmp::encode::write_u32(w, 0)?;
                        rmp::encode::write_bin(w, &bytes)?;
                    }
                    NodeInfo::Function(func_info) => {
                        rmp::encode::write_array_len(w, 2)?;
                        rmp::encode::write_u32(w, 1)?;
                        rmp::encode::write_str(w, &func_info.name)?;
                    }
                    NodeInfo::NestedCall { workflow_name } => {
                        rmp::encode::write_array_len(w, 2)?;
                        rmp::encode::write_u32(w, 2)?;
                        rmp::encode::write_str(w, workflow_name)?;
                    }
                }
            }
        }
        // "input_labels": sparse array of [node_idx, local_input_idx, label], empty labels skipped.
        rmp::encode::write_str(w, "input_labels")?;
        {
            let input_labels = self.input_labels.try_borrow()?;
            let n = input_labels.iter().filter(|l| !l.is_empty()).count();
            rmp::encode::write_array_len(w, n as u32)?;
            for (ih_idx, label) in input_labels.iter().enumerate() {
                if !label.is_empty() {
                    let node_idx = self.graph.inputs[ih_idx].node.idx as u32;
                    let local_idx = ih_local[ih_idx];
                    rmp::encode::write_array_len(w, 3)?;
                    rmp::encode::write_u32(w, node_idx)?;
                    rmp::encode::write_u32(w, local_idx)?;
                    rmp::encode::write_str(w, label)?;
                }
            }
        }
        // "output_labels": sparse array of [node_idx, local_output_idx, label], empty labels skipped.
        rmp::encode::write_str(w, "output_labels")?;
        {
            let output_labels = self.output_labels.try_borrow()?;
            let n = output_labels.iter().filter(|l| !l.is_empty()).count();
            rmp::encode::write_array_len(w, n as u32)?;
            for (oh_idx, label) in output_labels.iter().enumerate() {
                if !label.is_empty() {
                    let node_idx = self.graph.outputs[oh_idx].node.idx as u32;
                    let local_idx = oh_local[oh_idx];
                    rmp::encode::write_array_len(w, 3)?;
                    rmp::encode::write_u32(w, node_idx)?;
                    rmp::encode::write_u32(w, local_idx)?;
                    rmp::encode::write_str(w, label)?;
                }
            }
        }
        // "node_comments": sparse map {node_idx -> comment}, empty strings skipped.
        rmp::encode::write_str(w, "node_comments")?;
        {
            let node_comments = self.node_comments.try_borrow()?;
            let n_comments = node_comments.iter().filter(|c| !c.is_empty()).count();
            rmp::encode::write_map_len(w, n_comments as u32)?;
            for (i, comment) in node_comments.iter().enumerate() {
                if !comment.is_empty() {
                    rmp::encode::write_u32(w, i as u32)?;
                    rmp::encode::write_str(w, comment)?;
                }
            }
        }
        // "workflow_outputs": array of [node_idx, local_output_idx, name].
        rmp::encode::write_str(w, "workflow_outputs")?;
        rmp::encode::write_array_len(w, self.workflow_outputs.len() as u32)?;
        for (oh, name) in &self.workflow_outputs {
            let node_idx = self.graph.outputs[oh.idx].node.idx as u32;
            let local_idx = oh_local[oh.idx];
            rmp::encode::write_array_len(w, 3)?;
            rmp::encode::write_u32(w, node_idx)?;
            rmp::encode::write_u32(w, local_idx)?;
            rmp::encode::write_str(w, name)?;
        }
        // "workflow_input_names": array of strings.
        rmp::encode::write_str(w, "workflow_input_names")?;
        rmp::encode::write_array_len(w, self.workflow_input_names.len() as u32)?;
        for name in &self.workflow_input_names {
            rmp::encode::write_str(w, name)?;
        }
        // "workflow_input_index": array of [node_idx, local_input_idx, workflow_input_idx].
        rmp::encode::write_str(w, "workflow_input_index")?;
        {
            let wf_input_index = self.workflow_input_index.try_borrow()?;
            let n_mapped = wf_input_index.iter().filter(|o| o.is_some()).count();
            rmp::encode::write_array_len(w, n_mapped as u32)?;
            for (ih_idx, opt) in wf_input_index.iter().enumerate() {
                if let Some(wf_idx) = opt {
                    let node_idx = self.graph.inputs[ih_idx].node.idx as u32;
                    let local_idx = ih_local[ih_idx];
                    rmp::encode::write_array_len(w, 3)?;
                    rmp::encode::write_u32(w, node_idx)?;
                    rmp::encode::write_u32(w, local_idx)?;
                    rmp::encode::write_u32(w, *wf_idx as u32)?;
                }
            }
        }
        // "nested_workflows": array of [name, workflow].
        rmp::encode::write_str(w, "nested_workflows")?;
        rmp::encode::write_array_len(w, self.nested_workflows.len() as u32)?;
        for (name, wf) in &self.nested_workflows {
            rmp::encode::write_array_len(w, 2)?;
            rmp::encode::write_str(w, name)?;
            wf.write_to_msgpack(plugin_set, arena, w)?;
        }
        Ok(())
    }
}

impl Graph {
    const GRAPH_MSGPACK_VERSION_CURRENT: u64 = 1;

    pub fn read_from_msgpack(_src: &mut impl RmpRead) -> Result<Self, DagError> {
        todo!();
    }

    pub fn write_to_msgpack(
        &self,
        w: &mut impl RmpWrite,
        oh_local: &mut Vec<u32>,
        ih_local: &mut Vec<u32>,
    ) -> Result<(), DagError> {
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
        rmp::encode::write_u64(w, Self::GRAPH_MSGPACK_VERSION_CURRENT)?;
        // nodes: [[n_inputs, n_outputs], ...]
        rmp::encode::write_array_len(w, self.nodes.len() as u32)?;
        oh_local.clear();
        oh_local.resize(self.outputs.len(), 0);
        ih_local.clear();
        ih_local.resize(self.inputs.len(), 0);
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

struct NestedVec<T> {
    items: Vec<T>,
    offsets: Vec<usize>,
}

impl<E: RmpWriteErr> From<ValueWriteError<E>> for DagError {
    fn from(_: ValueWriteError<E>) -> Self {
        DagError::WriteError
    }
}
