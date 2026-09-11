use super::{DagError, Graph, Handle, IH, NH, NodeInfo, OH, Workflow};
use crate::{OrcHandle, PluginSet, TypeOwner};
use rmp::{
    decode::{RmpRead, RmpReadErr, ValueReadError},
    encode::{RmpWrite, RmpWriteErr, ValueWriteError},
};
use std::collections::HashSet;
use std::fmt::Write as _;

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
    /// This indicates the version of the serialized data. We write this version at the start when
    /// we serialize, and then read it back and check when deserializing. For now, if the versions
    /// don't match, we just error out, but in the future, after bumping up a few versions, I can
    /// dispatch to functions that deserialize the older versions.
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
                    3 => {
                        if arr_len != 2 {
                            return Err(DagError::ReadError);
                        }
                        let label = read_string(src)?;
                        NodeInfo::Inspect { label }
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
                    NodeInfo::Inspect { label } => {
                        rmp::encode::write_array_len(w, 2)?;
                        rmp::encode::write_u32(w, 3)?;
                        rmp::encode::write_str(w, label)?;
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

    /// Generates a Python source script that reconstructs this workflow's computation, in the
    /// `pyorc` dialect (`import orc`; plugin functions called as `orc.<name>(...)`; nested
    /// workflows as `@orc.workflow_function`-decorated helper functions). The outermost function
    /// is named `name`, or `generated_workflow` if `name` is `None`; either way it's undecorated,
    /// ready to be handed to `orc.make_workflow(...)` by whoever uses the script -- matching how
    /// a plain (non-nested) workflow is authored in `workflows/gen_test.py`.
    ///
    /// Every node in the graph is emitted, including ones no declared output depends on -- this
    /// is a faithful transcription of the graph as authored, not a dead-code-eliminating
    /// optimizer. Only `Inspect` nodes are skipped, since they have no outputs and nothing to
    /// compute. A cycle is reported the same way `run` reports one, via `DagError::CycleDetected`.
    ///
    /// A convenience wrapper around `write_python_script` for the common case of just wanting
    /// the result as a `String` in memory; write directly to a file (or any other
    /// `std::io::Write` sink) via `write_python_script` instead if that's not what's needed.
    pub fn to_python_script(&self, name: Option<&str>) -> Result<String, DagError> {
        let mut buf = Vec::new();
        self.write_python_script(name, &mut buf)?;
        String::from_utf8(buf).map_err(|_| DagError::WriteError)
    }

    /// Same as `to_python_script`, but writes directly into `out` -- a file, a `Vec<u8>`,
    /// anything implementing `std::io::Write` -- instead of building the whole script in memory
    /// as a `String` first.
    pub fn write_python_script(
        &self,
        name: Option<&str>,
        out: &mut impl std::io::Write,
    ) -> Result<(), DagError> {
        let mut emitted = HashSet::new();
        self.write_python_function(
            out,
            name.unwrap_or("generated_workflow"),
            false,
            &mut emitted,
        )
    }

    /// Topological order over *every* node in the graph, not just ones reachable from
    /// `workflow_outputs` (unlike `run`) -- a dead/unused branch still needs to be transcribed.
    /// Otherwise mirrors `run`'s own Euler-tour cycle detection exactly: a node revisited while
    /// still on the current path closes a cycle.
    fn topological_order(&self) -> Result<Vec<NH>, DagError> {
        let mut finished = HashSet::new();
        let mut on_path = HashSet::new();
        let mut order = Vec::new();
        for root in self.node_iter() {
            if finished.contains(&root) {
                continue;
            }
            let mut stack = vec![(root, false)];
            while let Some((node, visited_children)) = stack.pop() {
                if finished.contains(&node) {
                    continue;
                }
                if visited_children {
                    order.push(node);
                    finished.insert(node);
                    on_path.remove(&node);
                } else {
                    if on_path.contains(&node) {
                        return Err(DagError::CycleDetected);
                    }
                    stack.push((node, true));
                    on_path.insert(node);
                    for ih in self.node_inputs(node) {
                        if let Some(oh) = self.input_source(ih) {
                            stack.push((self.node_from_output(oh), false));
                        }
                    }
                }
            }
        }
        Ok(order)
    }

    /// The Python variable name a node's output pin should be referred to by: its `output_label`
    /// when one has been set, otherwise a synthetic `r_{index}` (matching the register-naming
    /// convention of a typical flat-IR-to-source emitter). Pin labels are essentially never set
    /// on a plain plugin-function call today (pyorc's tracer never calls `set_output_label`), so
    /// the synthetic fallback is the common case in practice, not just an edge case.
    fn output_var_name(&self, oh: OH) -> Result<String, DagError> {
        let label = self.output_label(oh)?;
        Ok(if label.is_empty() {
            format!("r_{}", oh.index())
        } else {
            label
        })
    }

    /// The Python expression to use for `ih`'s current value: the upstream node's own output
    /// variable if connected, the enclosing function's own parameter if `ih` is a dangling pin
    /// registered as a workflow input, or `None` for a pin that's dangling and not registered
    /// (there is nothing meaningful to reference).
    fn input_value_expr(&self, ih: IH) -> Result<String, DagError> {
        match self.input_source(ih) {
            Some(oh) => self.output_var_name(oh),
            None => match self.workflow_input_position(ih)? {
                Some(i) => Ok(self
                    .input_names()
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("arg_{i}"))),
                None => Ok("None".to_string()),
            },
        }
    }

    /// Appends one call statement for `nh` (a `Function` or `NestedCall` node) to `body`:
    /// `name0, name1 = callee(arg0, arg1)`, tuple-style for more than one output, or a bare
    /// `callee(arg0)` expression statement if it has no outputs at all.
    fn write_call(
        &self,
        body: &mut impl std::fmt::Write,
        nh: NH,
        callee: &str,
    ) -> Result<(), DagError> {
        let args = self
            .node_inputs(nh)
            .map(|ih| self.input_value_expr(ih))
            .collect::<Result<Vec<_>, _>>()?;
        let names = self
            .node_outputs(nh)
            .map(|oh| self.output_var_name(oh))
            .collect::<Result<Vec<_>, _>>()?;
        if names.is_empty() {
            writeln!(body, "    {callee}({})", args.join(", "))
        } else {
            writeln!(
                body,
                "    {} = {callee}({})",
                names.join(", "),
                args.join(", ")
            )
        }
        .map_err(|_| DagError::WriteError)
    }

    /// Emits `fn_name`'s own `def` into `out` -- decorated with `@orc.workflow_function` when
    /// `decorate` is true, which every nested workflow needs so it survives a round trip back
    /// through `orc.make_workflow`, but the outermost function does not (see `to_python_script`).
    /// A `NestedCall` node's referenced workflow is recursively emitted (memoized by name in
    /// `emitted`, shared across the whole recursion) immediately before the first line that calls
    /// it, so a nested function's own `def` always textually precedes its caller's.
    fn write_python_function(
        &self,
        out: &mut impl std::io::Write,
        fn_name: &str,
        decorate: bool,
        emitted: &mut HashSet<String>,
    ) -> Result<(), DagError> {
        let node_infos = self.node_infos.try_borrow()?;
        let order = self.topological_order()?;
        let mut body = String::new();
        for nh in order {
            match &node_infos[nh] {
                NodeInfo::Inspect { .. } => continue,
                NodeInfo::Constant(handle) => {
                    let oh = self
                        .node_outputs(nh)
                        .next()
                        .ok_or(DagError::InvalidOutputs)?;
                    let name = self.output_var_name(oh)?;
                    let literal = constant_literal(handle)?;
                    writeln!(body, "    {name} = {literal}").map_err(|_| DagError::WriteError)?;
                }
                NodeInfo::Function(func_info) => {
                    self.write_call(&mut body, nh, &format!("orc.{}", func_info.name))?;
                }
                NodeInfo::NestedCall { workflow_name } => {
                    if !emitted.contains(workflow_name) {
                        emitted.insert(workflow_name.clone());
                        let nested = self
                            .nested_workflows
                            .get(workflow_name)
                            .ok_or(DagError::InvalidFunction)?;
                        nested.write_python_function(out, workflow_name, true, emitted)?;
                    }
                    self.write_call(&mut body, nh, workflow_name)?;
                }
            }
        }

        let return_names = self
            .workflow_outputs()
            .iter()
            .map(|(oh, _name)| self.output_var_name(*oh))
            .collect::<Result<Vec<_>, _>>()?;

        if decorate {
            writeln!(out, "@orc.workflow_function").map_err(|_| DagError::WriteError)?;
        }
        writeln!(out, "def {fn_name}({}):", self.input_names().join(", "))
            .map_err(|_| DagError::WriteError)?;
        if body.is_empty() && return_names.is_empty() {
            writeln!(out, "    pass").map_err(|_| DagError::WriteError)?;
        } else {
            out.write_all(body.as_bytes())
                .map_err(|_| DagError::WriteError)?;
            if !return_names.is_empty() {
                writeln!(out, "    return {}", return_names.join(", "))
                    .map_err(|_| DagError::WriteError)?;
            }
        }
        writeln!(out).map_err(|_| DagError::WriteError)?;
        Ok(())
    }
}

/// Formats one leaf item's value as Python source text. Integer types just use their `Display`
/// output; floats always keep an explicit decimal point (`1` -> `1.0`) so the generated literal
/// reads unambiguously as a float regardless of how `orc.make_deck`'s dtype-driven construction
/// would otherwise coerce a bare integer literal.
trait PythonLiteral {
    fn python_literal(&self) -> String;
}

macro_rules! impl_int_python_literal {
    ($($t:ty),*) => {
        $(impl PythonLiteral for $t {
            fn python_literal(&self) -> String {
                self.to_string()
            }
        })*
    };
}
impl_int_python_literal!(u8, u16, u32, u64, i8, i16, i32, i64);

macro_rules! impl_float_python_literal {
    ($($t:ty),*) => {
        $(impl PythonLiteral for $t {
            fn python_literal(&self) -> String {
                let s = self.to_string();
                if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("nan") {
                    s
                } else {
                    format!("{s}.0")
                }
            }
        })*
    };
}
impl_float_python_literal!(f32, f64);

/// Recursively renders one `DeckView` as nested Python list syntax -- `[1.0, 2.0]`,
/// `[[1, 2], [3, 4]]`, or (at depth 0) a bare scalar with no brackets at all -- matching exactly
/// the nesting depth `orc.make_deck` would need to reconstruct the same structure.
fn deck_view_literal<T: Default + PythonLiteral>(view: crate::DeckView<'_, T>) -> String {
    if view.depth() == 0 {
        view.as_ref().python_literal()
    } else {
        let parts: Vec<String> = view.child().advance_iter().map(deck_view_literal).collect();
        format!("[{}]", parts.join(", "))
    }
}

fn deck_literal<T: crate::TOrcData + Default + PythonLiteral>(
    handle: &OrcHandle,
) -> Result<String, DagError> {
    let view = crate::DeckView::<T>::from_handle(handle).map_err(DagError::SdkError)?;
    Ok(deck_view_literal(view))
}

/// Renders a `Constant` node's handle as an `orc.make_deck(...)` call, preserving both its exact
/// nested structure (via `deck_view_literal`) and its exact scalar type via an explicit `dtype`
/// kwarg (never omitted, even for the `f64` default, to avoid depending on whatever `make_deck`
/// would infer from a bare Python literal with no dtype hint). Errors on any type this crate
/// doesn't own (a plugin-defined type), since there's no generic way to know how to spell an
/// arbitrary plugin type as a Python literal.
fn constant_literal(handle: &OrcHandle) -> Result<String, DagError> {
    let (literal, dtype) = match handle.type_id {
        crate::ORC_TYPE_U8 => (deck_literal::<u8>(handle)?, "u8"),
        crate::ORC_TYPE_U16 => (deck_literal::<u16>(handle)?, "u16"),
        crate::ORC_TYPE_U32 => (deck_literal::<u32>(handle)?, "u32"),
        crate::ORC_TYPE_U64 => (deck_literal::<u64>(handle)?, "u64"),
        crate::ORC_TYPE_I8 => (deck_literal::<i8>(handle)?, "i8"),
        crate::ORC_TYPE_I16 => (deck_literal::<i16>(handle)?, "i16"),
        crate::ORC_TYPE_I32 => (deck_literal::<i32>(handle)?, "i32"),
        crate::ORC_TYPE_I64 => (deck_literal::<i64>(handle)?, "i64"),
        crate::ORC_TYPE_F32 => (deck_literal::<f32>(handle)?, "f32"),
        crate::ORC_TYPE_F64 => (deck_literal::<f64>(handle)?, "f64"),
        other => return Err(DagError::UnsupportedConstantType(other)),
    };
    Ok(format!("orc.make_deck({literal}, dtype=\"{dtype}\")"))
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

#[cfg(test)]
mod test {
    use crate::{
        DagError, FuncInfo, IH, NodeInfo, OH, OrcHandle, PluginSet, Workflow,
        dag::Handle,
        host::test_harness::{PLUGINS, TestHarness},
        orc_dag,
    };

    fn assert_workflows_equal(a: &Workflow, b: &Workflow) {
        // Graph topology
        assert_eq!(a.graph.nodes.len(), b.graph.nodes.len(), "node count");
        assert_eq!(a.graph.inputs.len(), b.graph.inputs.len(), "input count");
        assert_eq!(a.graph.outputs.len(), b.graph.outputs.len(), "output count");
        assert_eq!(a.graph.links.len(), b.graph.links.len(), "link count");
        // Node infos
        {
            let a_infos = a.node_infos.try_borrow().unwrap();
            let b_infos = b.node_infos.try_borrow().unwrap();
            for (ai, bi) in a_infos.iter().zip(b_infos.iter()) {
                match (ai, bi) {
                    (NodeInfo::Constant(ah), NodeInfo::Constant(bh)) => {
                        assert_eq!(ah.type_id, bh.type_id, "constant type_id");
                        assert_eq!(ah.n_items, bh.n_items, "constant n_items");
                    }
                    (NodeInfo::Function(af), NodeInfo::Function(bf)) => {
                        assert_eq!(af.name, bf.name, "function name");
                        assert_eq!(af.desc, bf.desc, "function desc");
                        assert_eq!(af.n_inputs, bf.n_inputs, "function n_inputs");
                        assert_eq!(af.n_outputs, bf.n_outputs, "function n_outputs");
                    }
                    (
                        NodeInfo::NestedCall {
                            workflow_name: an, ..
                        },
                        NodeInfo::NestedCall {
                            workflow_name: bn, ..
                        },
                    ) => {
                        assert_eq!(an, bn, "nested call name");
                    }
                    _ => panic!("node info variant mismatch"),
                }
            }
        }
        // Input labels
        {
            let a_labels = a.input_labels.try_borrow().unwrap();
            let b_labels = b.input_labels.try_borrow().unwrap();
            for (al, bl) in a_labels.iter().zip(b_labels.iter()) {
                assert_eq!(al, bl, "input label");
            }
        }
        // Output labels
        {
            let a_labels = a.output_labels.try_borrow().unwrap();
            let b_labels = b.output_labels.try_borrow().unwrap();
            for (al, bl) in a_labels.iter().zip(b_labels.iter()) {
                assert_eq!(al, bl, "output label");
            }
        }
        // Node comments
        {
            let a_comments = a.node_comments.try_borrow().unwrap();
            let b_comments = b.node_comments.try_borrow().unwrap();
            for (ac, bc) in a_comments.iter().zip(b_comments.iter()) {
                assert_eq!(ac, bc, "node comment");
            }
        }
        // Workflow outputs
        assert_eq!(
            a.workflow_outputs.len(),
            b.workflow_outputs.len(),
            "workflow_outputs count"
        );
        for ((ao, an), (bo, bn)) in a.workflow_outputs.iter().zip(b.workflow_outputs.iter()) {
            assert_eq!(ao.idx, bo.idx, "workflow output handle idx");
            assert_eq!(an, bn, "workflow output name");
        }
        // Workflow input names
        assert_eq!(
            a.workflow_input_names, b.workflow_input_names,
            "workflow_input_names"
        );
        // Workflow input index
        {
            let a_idx = a.workflow_input_index.try_borrow().unwrap();
            let b_idx = b.workflow_input_index.try_borrow().unwrap();
            for (ai, bi) in a_idx.iter().zip(b_idx.iter()) {
                assert_eq!(ai, bi, "workflow_input_index");
            }
        }
        // Nested workflows
        assert_eq!(
            a.nested_workflows.len(),
            b.nested_workflows.len(),
            "nested workflow count"
        );
        for ((an, aw), (bn, bw)) in a.nested_workflows.iter().zip(b.nested_workflows.iter()) {
            assert_eq!(an, bn, "nested workflow name");
            assert_workflows_equal(aw, bw);
        }
    }

    // ==================== roundtrip: happy path ====================

    #[test]
    fn t_roundtrip_empty_workflow() {
        let h = TestHarness::new();
        let wf = Workflow::default();
        assert_eq!(wf.graph.nodes.len(), 0);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_single_constant() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let c (const [1.0f64, 2.0, 3.0]))
            (return c)
        })
        .unwrap();
        // Pre-conditions
        assert_eq!(wf.graph.nodes.len(), 1);
        assert_eq!(wf.workflow_outputs.len(), 1);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        // Post: constant data preserved
        let infos = wf2.node_infos.try_borrow().unwrap();
        match &infos.iter().next().unwrap() {
            NodeInfo::Constant(handle) => {
                assert_eq!(handle.n_items, 3);
                assert_eq!(handle.items::<f64>(), &[1.0, 2.0, 3.0]);
            }
            _ => panic!("expected Constant"),
        }
    }

    #[test]
    fn t_roundtrip_single_function() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 10.0f64))
            (let b (const 20.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        // Pre-conditions
        assert_eq!(wf.graph.nodes.len(), 3); // 2 consts + 1 add
        assert_eq!(wf.graph.links.len(), 2);
        assert_eq!(wf.workflow_outputs.len(), 1);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        // Post: function info recovered from plugin_set
        let infos = wf2.node_infos.try_borrow().unwrap();
        let add_info = infos.iter().find(|i| matches!(i, NodeInfo::Function(_)));
        match add_info.unwrap() {
            NodeInfo::Function(fi) => {
                assert_eq!(fi.name, "add");
                assert!(fi.func.is_some(), "func pointer recovered from plugin_set");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn t_roundtrip_chain_topology() {
        // const -> add -> mul -> output
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 2.0f64))
            (let b (const 3.0f64))
            (let sum (add a b))
            (let c (const 10.0f64))
            (let product (multiply sum c))
            (return product)
        })
        .unwrap();
        assert_eq!(wf.graph.nodes.len(), 5);
        assert_eq!(wf.graph.links.len(), 4);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_diamond_topology() {
        // const a, const b -> add -> sub -> out
        //                  -> mul -^
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 5.0f64))
            (let b (const 3.0f64))
            (let sum (add a b))
            (let product (multiply a b))
            (let diff (subtract sum product))
            (return diff)
        })
        .unwrap();
        // a feeds into both add and mul -> fan-out
        assert_eq!(wf.graph.nodes.len(), 5); // 2 consts + add + mul + sub
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_multi_output_function() {
        // complex_get_parts has 1 input, 2 outputs
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let re (const 1.0f64))
            (let im (const 2.0f64))
            (let c (create_complex re im))
            (let (real imag) (complex_get_parts c))
            (return real imag)
        })
        .unwrap();
        assert_eq!(wf.workflow_outputs.len(), 2);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_workflow_inputs() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let result (add 'x 'y))
            (return result)
        })
        .unwrap();
        // Pre-conditions
        assert_eq!(wf.workflow_input_names.len(), 2);
        assert_eq!(wf.workflow_input_names[0], "x");
        assert_eq!(wf.workflow_input_names[1], "y");
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_labels() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        // Set labels on the add node's inputs/outputs
        let add_node = crate::dag::NH { idx: 2 }; // add is 3rd node
        let add_in0 = wf.graph.node_inputs(add_node).next().unwrap();
        let add_in1 = wf.graph.node_inputs(add_node).nth(1).unwrap();
        let add_out = wf.graph.node_outputs(add_node).next().unwrap();
        wf.set_input_label(add_in0, "lhs".to_string()).unwrap();
        wf.set_input_label(add_in1, "rhs".to_string()).unwrap();
        wf.set_output_label(add_out, "sum".to_string()).unwrap();
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        // Verify labels explicitly
        assert_eq!(wf2.input_label(add_in0).unwrap(), "lhs");
        assert_eq!(wf2.input_label(add_in1).unwrap(), "rhs");
        assert_eq!(wf2.output_label(add_out).unwrap(), "sum");
    }

    #[test]
    fn t_roundtrip_node_comments() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        let add_node = crate::dag::NH { idx: 2 };
        wf.set_node_comment(add_node, "This adds two numbers")
            .unwrap();
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        assert_eq!(wf2.node_comment(add_node).unwrap(), "This adds two numbers");
    }

    #[test]
    fn t_roundtrip_nested_workflow() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (fn double_add
                (let sum (add 'a 'a))
                (return sum)
            )
            (let x (const 5.0f64))
            (let result (double_add x))
            (return result)
        })
        .unwrap();
        assert_eq!(wf.nested_workflows.len(), 1);
        assert!(wf.nested_workflows.contains_key("double_add"));
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_multiple_nested_workflows() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (fn square
                (let result (multiply 'v 'v))
                (return result)
            )
            (fn double
                (let result (add 'v 'v))
                (return result)
            )
            (let x (const 3.0f64))
            (let sq (square x))
            (let db (double sq))
            (return db)
        })
        .unwrap();
        assert_eq!(wf.nested_workflows.len(), 2);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        // BTreeMap order: "double" < "square"
        let keys: Vec<_> = wf2.nested_workflows.keys().collect();
        assert_eq!(keys, vec!["double", "square"]);
    }

    #[test]
    fn t_roundtrip_integer_constant() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let c (const 42i64))
            (return c)
        })
        .unwrap();
        let wf2 = h.roundtrip(&wf);
        let infos = wf2.node_infos.try_borrow().unwrap();
        match &infos.iter().next().unwrap() {
            NodeInfo::Constant(handle) => {
                assert_eq!(handle.n_items, 1);
                assert_eq!(handle.items::<i64>(), &[42]);
            }
            _ => panic!("expected Constant"),
        }
    }

    #[test]
    fn t_roundtrip_nested_deck_constant() {
        // Nested deck: [[1.0, 2.0], [3.0]]
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let c (const [[1.0f64, 2.0], [3.0]]))
            (return c)
        })
        .unwrap();
        let wf2 = h.roundtrip(&wf);
        let infos = wf2.node_infos.try_borrow().unwrap();
        match &infos.iter().next().unwrap() {
            NodeInfo::Constant(handle) => {
                assert_eq!(handle.n_items, 3);
                assert_eq!(handle.items::<f64>(), &[1.0, 2.0, 3.0]);
                assert!(handle.n_marks > 0, "nested deck should have marks");
            }
            _ => panic!("expected Constant"),
        }
    }

    #[test]
    fn t_roundtrip_deterministic_bytes() {
        // Serializing the same workflow twice produces identical bytes.
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        let bytes1 = h.roundtrip_bytes(&wf);
        let bytes2 = h.roundtrip_bytes(&wf);
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn t_roundtrip_double_roundtrip() {
        // wf -> bytes -> wf2 -> bytes2 must produce identical bytes.
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (fn inner
                (let result (multiply 'a 'a))
                (return result)
            )
            (let x (const 7.0f64))
            (let result (inner x))
            (return result)
        })
        .unwrap();
        let bytes1 = h.roundtrip_bytes(&wf);
        let wf2 = h.read_workflow(&bytes1).unwrap();
        let bytes2 = h.roundtrip_bytes(&wf2);
        assert_eq!(bytes1, bytes2);
    }

    // ==================== error cases ====================

    #[test]
    fn t_read_empty_buffer_fails() {
        let h = TestHarness::new();
        let result = h.read_workflow(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn t_read_truncated_buffer_fails() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        let bytes = h.roundtrip_bytes(&wf);
        // Try various truncation points
        for len in [1, 5, 10, bytes.len() / 2, bytes.len() - 1] {
            let result = h.read_workflow(&bytes[..len]);
            assert!(result.is_err(), "truncation at {} should fail", len);
        }
    }

    #[test]
    fn t_read_wrong_version_fails() {
        let h = TestHarness::new();
        // Ensure empty workflow serializes.
        let wf = Workflow::default();
        let mut buf = Vec::new();
        wf.write_to_msgpack(&PLUGINS, &h.arena, &mut buf).unwrap();
        // Find and corrupt the version field. The version is the first u64 value
        // after "version" key. Write a custom bad version message.
        let mut bad_buf = Vec::new();
        rmp::encode::write_map_len(&mut bad_buf, 10).unwrap();
        rmp::encode::write_str(&mut bad_buf, "version").unwrap();
        rmp::encode::write_u64(&mut bad_buf, 999).unwrap(); // bad version
        let result = h.read_workflow(&bad_buf);
        assert!(matches!(result, Err(DagError::VersionMismatch)));
    }

    #[test]
    fn t_read_wrong_key_order_fails() {
        let h = TestHarness::new();
        let mut bad_buf = Vec::new();
        rmp::encode::write_map_len(&mut bad_buf, 10).unwrap();
        // Write "graph" first instead of "version"
        rmp::encode::write_str(&mut bad_buf, "graph").unwrap();
        let result = h.read_workflow(&bad_buf);
        assert!(matches!(result, Err(DagError::ReadError)));
    }

    #[test]
    fn t_read_unknown_function_fails() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        let mut bytes = h.roundtrip_bytes(&wf);
        // Replace "add" with "zzz" in the msgpack bytes.
        // Find the "add" string in the serialized bytes and replace it with "zzz".
        let add_bytes = b"\xa3add"; // fixstr(3) + "add"
        let zzz_bytes = b"\xa3zzz";
        let pos = bytes
            .windows(4)
            .position(|w| w == add_bytes)
            .expect("should find 'add' in bytes");
        bytes[pos..pos + 4].copy_from_slice(zzz_bytes);
        let result = h.read_workflow(&bytes);
        assert!(matches!(result, Err(DagError::InvalidFunction)));
    }

    #[test]
    fn t_roundtrip_many_constants() {
        // Stress test with many nodes
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        // Build manually: 10 constants chained through adds
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let c (const 3.0f64))
            (let d (const 4.0f64))
            (let e (const 5.0f64))
            (let ab (add a b))
            (let cd (add c d))
            (let abcd (add ab cd))
            (let result (add abcd e))
            (return result)
        })
        .unwrap();
        assert_eq!(wf.graph.nodes.len(), 9);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_workflow_with_all_features() {
        // Comprehensive test: constants, functions, labels, comments, inputs, outputs, nested
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (fn scale
                (let result (multiply 'val 'factor))
                (return result)
            )
            (let bias (const 100.0f64))
            (let scaled (scale 'input bias))
            (let result (add scaled bias))
            (return result)
        })
        .unwrap();
        // Add labels and comments
        let scale_node = crate::dag::NH { idx: 1 }; // scale call
        wf.set_node_comment(scale_node, "Scales the input").unwrap();
        let add_node = crate::dag::NH { idx: 2 }; // add
        wf.set_node_comment(add_node, "Adds bias").unwrap();
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_empty_string_labels_not_serialized() {
        // Empty labels should not be serialized (sparse encoding)
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        // Only set one label, leave others empty
        let add_node = crate::dag::NH { idx: 2 };
        let first_input = wf.graph.node_inputs(add_node).next().unwrap();
        wf.set_input_label(first_input, "only_this_one".to_string())
            .unwrap();
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        // The other inputs should still have empty labels
        let second_input = wf.graph.node_inputs(add_node).nth(1).unwrap();
        assert_eq!(wf2.input_label(second_input).unwrap(), "");
    }

    #[test]
    fn t_roundtrip_unicode_labels_and_comments() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let result (add a b))
            (return result)
        })
        .unwrap();
        let add_node = crate::dag::NH { idx: 2 };
        wf.set_node_comment(add_node, "足し算 🧮").unwrap();
        let input = wf.graph.node_inputs(add_node).next().unwrap();
        wf.set_input_label(input, "入力α".to_string()).unwrap();
        let wf2 = h.roundtrip(&wf);
        assert_eq!(wf2.node_comment(add_node).unwrap(), "足し算 🧮");
        assert_eq!(wf2.input_label(input).unwrap(), "入力α");
    }

    #[test]
    fn t_roundtrip_no_outputs() {
        // A workflow with nodes but no set_outputs
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let _result (add a b))
        })
        .unwrap();
        assert_eq!(wf.workflow_outputs.len(), 0);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
        assert_eq!(wf2.workflow_outputs.len(), 0);
    }

    #[test]
    fn t_roundtrip_multiple_outputs() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let a (const 1.0f64))
            (let b (const 2.0f64))
            (let sum (add a b))
            (let product (multiply a b))
            (return sum product)
        })
        .unwrap();
        assert_eq!(wf.workflow_outputs.len(), 2);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_constant_only_no_functions() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let c (const 42.0f64))
            (return c)
        })
        .unwrap();
        // No function nodes, no links
        assert_eq!(wf.graph.links.len(), 0);
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    #[test]
    fn t_roundtrip_fan_out() {
        // One constant feeding into multiple function inputs
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        orc_dag!(&*PLUGINS, &h.handle_counter, &h.registry, &mut wf, {
            (let x (const 5.0f64))
            (let sum (add x x))
            (let product (multiply x x))
            (let result (subtract sum product))
            (return result)
        })
        .unwrap();
        // x feeds into add(x,x), mul(x,x) — 4 links from the same constant output
        let wf2 = h.roundtrip(&wf);
        assert_workflows_equal(&wf, &wf2);
    }

    // ==================== to_python_script ====================

    fn make_func_info(name: &str, n_in: usize, n_out: usize) -> FuncInfo {
        FuncInfo {
            name: name.to_string(),
            desc: String::new(),
            n_inputs: Some(n_in),
            n_outputs: Some(n_out),
            input_args: Default::default(),
            output_args: Default::default(),
            func: None,
        }
    }

    fn constant_handle<T>(h: &TestHarness, deck: crate::Deck<T>) -> OrcHandle
    where
        T: crate::TOrcData + Default + Send + Sync + 'static,
    {
        let mut handle = OrcHandle {
            handle: h
                .handle_counter
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        };
        h.registry
            .alloc_with_value(Some(deck), &mut handle)
            .unwrap();
        handle
    }

    #[test]
    fn t_to_python_script_simple_function_chain() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        let lhs = constant_handle(&h, crate::Deck::from_value(3.0f64));
        let rhs = constant_handle(&h, crate::Deck::from_value(4.0f64));
        let (_, lhs_out) = wf.add_constant(lhs).unwrap();
        let (_, rhs_out) = wf.add_constant(rhs).unwrap();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        wf.add_function(make_func_info("add", 2, 1), &mut ins, &mut outs)
            .unwrap();
        wf.connect(lhs_out, ins[0]).unwrap();
        wf.connect(rhs_out, ins[1]).unwrap();
        wf.set_outputs(&[(outs[0], String::new())]).unwrap();

        let script = wf.to_python_script(None).unwrap();
        let expected = format!(
            "def generated_workflow():\n    r_{a} = orc.make_deck(3.0, dtype=\"f64\")\n    r_{b} = orc.make_deck(4.0, dtype=\"f64\")\n    r_{c} = orc.add(r_{a}, r_{b})\n    return r_{c}\n\n",
            a = lhs_out.index(),
            b = rhs_out.index(),
            c = outs[0].index(),
        );
        assert_eq!(script, expected);
    }

    #[test]
    fn t_to_python_script_uses_the_given_name_when_provided() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default()];
        wf.add_function(make_func_info("source", 0, 1), &mut [], &mut outs)
            .unwrap();
        wf.set_outputs(&[(outs[0], String::new())]).unwrap();

        let script = wf.to_python_script(Some("my_workflow")).unwrap();
        assert!(script.starts_with("def my_workflow():\n"), "got:\n{script}");
        assert!(
            !script.contains("generated_workflow"),
            "the default name must not leak in when a name is given:\n{script}"
        );
    }

    #[test]
    fn t_to_python_script_uses_output_label_when_set() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default()];
        wf.add_function(make_func_info("source", 0, 1), &mut [], &mut outs)
            .unwrap();
        wf.set_output_label(outs[0], "my_value".to_string())
            .unwrap();
        wf.set_outputs(&[(outs[0], String::new())]).unwrap();

        let script = wf.to_python_script(None).unwrap();
        assert_eq!(
            script,
            "def generated_workflow():\n    my_value = orc.source()\n    return my_value\n\n"
        );
    }

    #[test]
    fn t_to_python_script_detects_a_cycle() {
        let mut wf = Workflow::default();
        let mut a_in = [IH::default()];
        let mut a_out = [OH::default()];
        wf.add_function(make_func_info("a", 1, 1), &mut a_in, &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let mut b_out = [OH::default()];
        wf.add_function(make_func_info("b", 1, 1), &mut b_in, &mut b_out)
            .unwrap();
        wf.connect(a_out[0], b_in[0]).unwrap();
        wf.connect(b_out[0], a_in[0]).unwrap();
        assert!(matches!(
            wf.to_python_script(None),
            Err(DagError::CycleDetected)
        ));
    }

    #[test]
    fn t_to_python_script_emits_a_nested_workflow_function_before_its_caller() {
        let h = TestHarness::new();
        let mut inner = Workflow::default();
        let mut inner_ins = [IH::default(); 2];
        let mut inner_outs = [OH::default(); 1];
        inner
            .add_function(make_func_info("add", 2, 1), &mut inner_ins, &mut inner_outs)
            .unwrap();
        inner
            .set_inputs(&[(inner_ins[0], 0, "a"), (inner_ins[1], 1, "b")])
            .unwrap();
        inner
            .set_outputs(&[(inner_outs[0], String::new())])
            .unwrap();

        let mut outer = Workflow::default();
        outer
            .push_nested_workflow("double".to_string(), inner, &PluginSet::default())
            .unwrap();
        let x = constant_handle(&h, crate::Deck::from_value(1.0f64));
        let y = constant_handle(&h, crate::Deck::from_value(2.0f64));
        let (_, x_out) = outer.add_constant(x).unwrap();
        let (_, y_out) = outer.add_constant(y).unwrap();
        let mut call_ins = [IH::default(); 2];
        let mut call_outs = [OH::default(); 1];
        outer
            .add_nested_workflow_call("double", &mut call_ins, &mut call_outs)
            .unwrap();
        outer.connect(x_out, call_ins[0]).unwrap();
        outer.connect(y_out, call_ins[1]).unwrap();
        outer.set_outputs(&[(call_outs[0], String::new())]).unwrap();

        let script = outer.to_python_script(None).unwrap();
        let decorator_pos = script
            .find("@orc.workflow_function")
            .expect("nested def must be decorated");
        let nested_def_pos = script
            .find("def double(a, b):")
            .expect("nested def must be emitted");
        let outer_def_pos = script
            .find("def generated_workflow():")
            .expect("outer def must be emitted");
        assert!(
            decorator_pos < nested_def_pos,
            "decorator must precede the nested def:\n{script}"
        );
        assert!(
            nested_def_pos < outer_def_pos,
            "nested def must precede its caller:\n{script}"
        );
        assert!(
            script.contains(&format!(
                "= double(r_{}, r_{})",
                x_out.index(),
                y_out.index()
            )),
            "got:\n{script}"
        );
    }

    #[test]
    fn t_to_python_script_emits_nodes_unreachable_from_any_output() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default()];
        wf.add_function(make_func_info("dead_end", 0, 1), &mut [], &mut outs)
            .unwrap();
        // No workflow outputs declared at all -- this node feeds nothing.
        let script = wf.to_python_script(None).unwrap();
        assert!(
            script.contains("orc.dead_end()"),
            "an unreachable node must still be transcribed:\n{script}"
        );
    }

    #[test]
    fn t_to_python_script_errors_on_an_opaque_constant_type() {
        let mut wf = Workflow::default();
        let handle = OrcHandle {
            type_id: 0xdead_beef,
            ..Default::default()
        };
        wf.add_constant(handle).unwrap();
        assert!(matches!(
            wf.to_python_script(None),
            Err(DagError::UnsupportedConstantType(0xdead_beef))
        ));
    }

    #[test]
    fn t_to_python_script_maps_workflow_inputs_to_parameters() {
        let mut wf = Workflow::default();
        let mut ins = [IH::default(); 1];
        let mut outs = [OH::default(); 1];
        wf.add_function(make_func_info("negate", 1, 1), &mut ins, &mut outs)
            .unwrap();
        wf.set_inputs(&[(ins[0], 0, "value")]).unwrap();
        wf.set_outputs(&[(outs[0], String::new())]).unwrap();

        let script = wf.to_python_script(None).unwrap();
        assert!(
            script.starts_with("def generated_workflow(value):\n"),
            "got:\n{script}"
        );
        assert!(script.contains("orc.negate(value)"), "got:\n{script}");
    }

    #[test]
    fn t_to_python_script_returns_a_tuple_for_multiple_workflow_outputs() {
        let mut wf = Workflow::default();
        let mut outs = [OH::default(); 2];
        wf.add_function(make_func_info("two_outs", 0, 2), &mut [], &mut outs)
            .unwrap();
        wf.set_outputs(&[(outs[0], String::new()), (outs[1], String::new())])
            .unwrap();

        let script = wf.to_python_script(None).unwrap();
        assert!(
            script.contains(&format!(
                "return r_{}, r_{}",
                outs[0].index(),
                outs[1].index()
            )),
            "got:\n{script}"
        );
    }

    #[test]
    fn t_to_python_script_renders_a_nested_list_constant() {
        let h = TestHarness::new();
        let mut wf = Workflow::default();
        let handle = constant_handle(&h, crate::deck![[1.0f64, 2.0], [3.0]]);
        let (_, oh) = wf.add_constant(handle).unwrap();
        wf.set_outputs(&[(oh, String::new())]).unwrap();

        let script = wf.to_python_script(None).unwrap();
        assert!(
            script.contains("orc.make_deck([[1.0, 2.0], [3.0]], dtype=\"f64\")"),
            "got:\n{script}"
        );
    }
}
