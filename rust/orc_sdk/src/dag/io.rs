use super::{DagError, Graph, IH, NH, NodeInfo, OH, Workflow};
use crate::{OrcHandle, PluginSet, TypeOwner};
use rmp::{
    decode::{RmpRead, RmpReadErr, ValueReadError},
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

    pub fn read_from_msgpack(
        src: &mut impl RmpRead,
        plugin_set: &PluginSet,
        registry: &crate::DeckRegistry,
        ctx: u64,
        next_handle_id: &mut impl FnMut() -> u64,
    ) -> Result<Self, DagError> {
        let _n_entries = rmp::decode::read_map_len(src)?;
        let mut wf = Workflow::default();
        let mut node_ihs = NestedVec::<IH>::default();
        let mut node_ohs = NestedVec::<OH>::default();
        // version
        expect_key(src, "version")?;
        let v = rmp::decode::read_u64(src)?;
        if v != Self::WORKFLOW_MSGPACK_VERSION_CURRENT {
            return Err(DagError::VersionMismatch);
        }
        // graph
        expect_key(src, "graph")?;
        wf.graph
            .read_from_msgpack(src, &mut node_ihs, &mut node_ohs)?;
        // node_infos
        expect_key(src, "node_infos")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            if n != wf.graph.nodes.len() {
                return Err(DagError::ReadError);
            }
            let mut node_infos = wf.node_infos.try_borrow_mut()?;
            for i in 0..n {
                let arr_len = rmp::decode::read_array_len(src)?;
                let tag = rmp::decode::read_u32(src)?;
                let info = match tag {
                    0 => {
                        if arr_len != 3 {
                            return Err(DagError::ReadError);
                        }
                        let type_id = rmp::decode::read_u64(src)?;
                        let bin_len = rmp::decode::read_bin_len(src)? as usize;
                        let mut buf = vec![0u8; bin_len];
                        src.read_exact_buf(&mut buf)
                            .map_err(|_| DagError::ReadError)?;
                        let mut handle = OrcHandle {
                            handle: next_handle_id(),
                            ..Default::default()
                        };
                        match plugin_set.get_type_owner(type_id) {
                            Some(TypeOwner::BuiltIn(_)) => {
                                let mut cursor = std::io::Cursor::new(&buf);
                                crate::try_deserialize_handle(&mut cursor, &mut handle, registry)
                                    .map_err(|_| DagError::ReadError)?;
                            }
                            Some(TypeOwner::Plugin(idx, _)) => {
                                plugin_set.plugins()[*idx]
                                    .deserialize_deck(ctx, &buf, &mut handle)
                                    .map_err(|_| DagError::ReadError)?;
                            }
                            None => return Err(DagError::ReadError),
                        }
                        if handle.type_id != type_id {
                            return Err(DagError::ReadError);
                        }
                        NodeInfo::Constant(handle)
                    }
                    1 => {
                        if arr_len != 2 {
                            return Err(DagError::ReadError);
                        }
                        let name = read_string(src)?;
                        let func_info = plugin_set
                            .get_function(&name)
                            .ok_or(DagError::InvalidFunction)?
                            .clone();
                        NodeInfo::Function(func_info)
                    }
                    2 => {
                        if arr_len != 2 {
                            return Err(DagError::ReadError);
                        }
                        let workflow_name = read_string(src)?;
                        NodeInfo::NestedCall { workflow_name }
                    }
                    _ => return Err(DagError::ReadError),
                };
                node_infos[NH { idx: i }] = info;
            }
        }
        // input_labels
        expect_key(src, "input_labels")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            let mut labels = wf.input_labels.try_borrow_mut()?;
            for _ in 0..n {
                if rmp::decode::read_array_len(src)? != 3 {
                    return Err(DagError::ReadError);
                }
                let node_idx = rmp::decode::read_u32(src)? as usize;
                let local_idx = rmp::decode::read_u32(src)? as usize;
                let ihs = node_ihs.get(node_idx)?;
                let &ih = ihs.get(local_idx).ok_or(DagError::ReadError)?;
                labels[ih] = read_string(src)?;
            }
        }
        // output_labels
        expect_key(src, "output_labels")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            let mut labels = wf.output_labels.try_borrow_mut()?;
            for _ in 0..n {
                if rmp::decode::read_array_len(src)? != 3 {
                    return Err(DagError::ReadError);
                }
                let node_idx = rmp::decode::read_u32(src)? as usize;
                let local_idx = rmp::decode::read_u32(src)? as usize;
                let ohs = node_ohs.get(node_idx)?;
                let &oh = ohs.get(local_idx).ok_or(DagError::ReadError)?;
                labels[oh] = read_string(src)?;
            }
        }
        // node_comments
        expect_key(src, "node_comments")?;
        {
            let n = rmp::decode::read_map_len(src)? as usize;
            let mut comments = wf.node_comments.try_borrow_mut()?;
            for _ in 0..n {
                let node_idx = rmp::decode::read_u32(src)? as usize;
                if node_idx >= wf.graph.nodes.len() {
                    return Err(DagError::ReadError);
                }
                comments[NH { idx: node_idx }] = read_string(src)?;
            }
        }
        // workflow_outputs
        expect_key(src, "workflow_outputs")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            let mut seen_ohs = std::collections::HashSet::new();
            for _ in 0..n {
                if rmp::decode::read_array_len(src)? != 3 {
                    return Err(DagError::ReadError);
                }
                let node_idx = rmp::decode::read_u32(src)? as usize;
                let local_idx = rmp::decode::read_u32(src)? as usize;
                let name = read_string(src)?;
                let ohs = node_ohs.get(node_idx)?;
                let &oh = ohs.get(local_idx).ok_or(DagError::ReadError)?;
                if !wf.graph.is_valid_output(oh) || !seen_ohs.insert(oh.idx) {
                    return Err(DagError::InvalidOutputs);
                }
                wf.workflow_outputs.push((oh, name));
            }
        }
        // workflow_input_names
        expect_key(src, "workflow_input_names")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            for _ in 0..n {
                wf.workflow_input_names.push(read_string(src)?);
            }
        }
        // workflow_input_index
        expect_key(src, "workflow_input_index")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            let mut wf_input_index = wf.workflow_input_index.try_borrow_mut()?;
            for _ in 0..n {
                if rmp::decode::read_array_len(src)? != 3 {
                    return Err(DagError::ReadError);
                }
                let node_idx = rmp::decode::read_u32(src)? as usize;
                let local_idx = rmp::decode::read_u32(src)? as usize;
                let wf_idx = rmp::decode::read_u32(src)? as usize;
                if wf_idx >= wf.workflow_input_names.len() {
                    return Err(DagError::InvalidInputs);
                }
                let ihs = node_ihs.get(node_idx)?;
                let &ih = ihs.get(local_idx).ok_or(DagError::ReadError)?;
                wf_input_index[ih] = Some(wf_idx);
            }
        }
        // nested_workflows
        expect_key(src, "nested_workflows")?;
        {
            let n = rmp::decode::read_array_len(src)? as usize;
            for _ in 0..n {
                if rmp::decode::read_array_len(src)? != 2 {
                    return Err(DagError::ReadError);
                }
                let name = read_string(src)?;
                let nested =
                    Workflow::read_from_msgpack(src, plugin_set, registry, ctx, next_handle_id)?;
                wf.nested_workflows.insert(name, nested);
            }
        }
        Ok(wf)
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
                        rmp::encode::write_array_len(w, 3)?;
                        rmp::encode::write_u32(w, 0)?;
                        rmp::encode::write_u64(w, handle.type_id)?;
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

    fn read_from_msgpack(
        &mut self,
        src: &mut impl RmpRead,
        node_ihs: &mut NestedVec<IH>,
        node_ohs: &mut NestedVec<OH>,
    ) -> Result<(), DagError> {
        node_ihs.clear();
        node_ohs.clear();
        let outer_len = rmp::decode::read_array_len(src)?;
        if outer_len != 3 {
            return Err(DagError::ReadError);
        }
        let version = rmp::decode::read_u64(src)?;
        if version != Self::GRAPH_MSGPACK_VERSION_CURRENT {
            return Err(DagError::VersionMismatch);
        }
        let n_nodes = rmp::decode::read_array_len(src)? as usize;
        for _ in 0..n_nodes {
            let node_len = rmp::decode::read_array_len(src)?;
            if node_len != 2 {
                return Err(DagError::ReadError);
            }
            let n_inputs = rmp::decode::read_u32(src)? as usize;
            let n_outputs = rmp::decode::read_u32(src)? as usize;
            let mut ihs = vec![IH::default(); n_inputs];
            let mut ohs = vec![OH::default(); n_outputs];
            self.push_node(&mut ihs, &mut ohs)?;
            node_ihs.push_slice(&ihs);
            node_ohs.push_slice(&ohs);
        }
        let n_links = rmp::decode::read_array_len(src)? as usize;
        for _ in 0..n_links {
            let link_len = rmp::decode::read_array_len(src)?;
            if link_len != 4 {
                return Err(DagError::ReadError);
            }
            let src_node = rmp::decode::read_u32(src)? as usize;
            let local_out = rmp::decode::read_u32(src)? as usize;
            let dst_node = rmp::decode::read_u32(src)? as usize;
            let local_in = rmp::decode::read_u32(src)? as usize;
            let oh = *node_ohs
                .get(src_node)?
                .get(local_out)
                .ok_or(DagError::ReadError)?;
            let ih = *node_ihs
                .get(dst_node)?
                .get(local_in)
                .ok_or(DagError::ReadError)?;
            self.push_link(oh, ih)?;
        }
        Ok(())
    }

    fn write_to_msgpack(
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

impl<T> Default for NestedVec<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            offsets: Vec::new(),
        }
    }
}

impl<T: Clone> NestedVec<T> {
    fn push_slice(&mut self, slice: &[T]) {
        self.items.extend_from_slice(slice);
        self.offsets.push(self.items.len());
    }

    fn get(&self, i: usize) -> Result<&[T], DagError> {
        let end = *self.offsets.get(i).ok_or(DagError::ReadError)?;
        let start = if i == 0 { 0 } else { self.offsets[i - 1] };
        Ok(&self.items[start..end])
    }

    fn clear(&mut self) {
        self.items.clear();
        self.offsets.clear();
    }
}

fn expect_key(src: &mut impl RmpRead, expected: &str) -> Result<(), DagError> {
    let key = read_string(src)?;
    if key != expected {
        return Err(DagError::ReadError);
    }
    Ok(())
}

fn read_string(src: &mut impl RmpRead) -> Result<String, DagError> {
    let len = rmp::decode::read_str_len(src)? as usize;
    let mut buf = vec![0u8; len];
    src.read_exact_buf(&mut buf)
        .map_err(|_| DagError::ReadError)?;
    String::from_utf8(buf).map_err(|_| DagError::ReadError)
}

impl<E: RmpReadErr> From<ValueReadError<E>> for DagError {
    fn from(_: ValueReadError<E>) -> Self {
        DagError::ReadError
    }
}

impl<E: RmpWriteErr> From<ValueWriteError<E>> for DagError {
    fn from(_: ValueWriteError<E>) -> Self {
        DagError::WriteError
    }
}
