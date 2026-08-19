use super::{
    DagError, Graph, IH, InputProperty, LH, LinkProperty, NH, NodeProperty, OH, OutputProperty,
    Property,
};
use crate::{FuncInfo, InputPropBuf, OrcHandle, OrcHandleBorrowed, PluginSet};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::atomic::{AtomicU64, Ordering},
};

pub enum NodeInfo {
    Constant { name: String, data: OrcHandle },
    Function(FuncInfo),
    NestedCall { workflow_name: String },
}

impl Clone for NodeInfo {
    fn clone(&self) -> Self {
        match self {
            // The host application is responsible for allocating the variable data manually.
            Self::Constant { name, data: _data } => Self::Constant {
                name: format!("{name}_copy"),
                data: OrcHandle::default(),
            },
            Self::Function(arg0) => Self::Function(arg0.clone()),
            Self::NestedCall { workflow_name } => Self::NestedCall {
                workflow_name: workflow_name.clone(),
            },
        }
    }
}

impl Default for NodeInfo {
    fn default() -> Self {
        Self::Function(FuncInfo::default())
    }
}

impl NodeInfo {
    pub fn name(&self) -> &str {
        match self {
            NodeInfo::Constant { name, .. } => name,
            NodeInfo::Function(func_info) => &func_info.name,
            NodeInfo::NestedCall { workflow_name } => &workflow_name,
        }
    }
}

pub struct Workflow {
    graph: Graph,
    node_infos: NodeProperty<NodeInfo>,
    input_labels: InputProperty<String>,
    output_labels: OutputProperty<String>,
    node_comments: NodeProperty<String>,
    workflow_outputs: Vec<(OH, String)>,
    workflow_input_index: InputProperty<Option<(usize, String)>>,
    nested_workflows: HashMap<String, Workflow>,
}

impl Default for Workflow {
    fn default() -> Self {
        let mut graph = Graph::default();
        let node_infos = graph.create_node_property(NodeInfo::default());
        let input_labels = graph.create_input_property(String::default());
        let output_labels = graph.create_output_property(String::default());
        let node_comments = graph.create_node_property(String::default());
        let workflow_input_index = graph.create_input_property(None);
        Self {
            graph,
            node_infos,
            input_labels,
            output_labels,
            node_comments,
            workflow_outputs: Default::default(),
            workflow_input_index,
            nested_workflows: Default::default(),
        }
    }
}

impl Workflow {
    pub fn with_capacity(
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Self {
        let mut graph = Graph::with_capacity(n_inputs, n_outputs, n_links, n_nodes);
        let node_infos =
            Property::with_capacity(n_nodes, &mut graph.node_props, NodeInfo::default());
        let input_labels = graph.create_input_property(String::default());
        let output_labels = graph.create_output_property(String::default());
        let node_comments = graph.create_node_property(String::default());
        let workflow_input_index = graph.create_input_property(None);
        Workflow {
            graph,
            node_infos,
            input_labels,
            output_labels,
            node_comments,
            workflow_outputs: Default::default(),
            workflow_input_index,
            nested_workflows: Default::default(),
        }
    }

    pub fn push_nested_workflow(
        &mut self,
        name: String,
        workflow: Workflow,
        plugin_set: &PluginSet,
    ) -> Result<(), DagError> {
        if let Some(_) = plugin_set.get_function(&name) {
            return Err(DagError::NamingConflict);
        }
        match self.nested_workflows.entry(name) {
            Entry::Occupied(_occupied) => return Err(DagError::NamingConflict),
            Entry::Vacant(vacant) => vacant.insert(workflow),
        };
        Ok(())
    }

    pub fn set_outputs(&mut self, outputs: &[(OH, String)]) -> Result<(), DagError> {
        if outputs
            .iter()
            .any(|(o, _name)| !self.graph.is_valid_output(*o))
        {
            return Err(DagError::InvalidOutputs);
        }
        {
            // Ensure the outputs are unique.
            let hset: HashSet<usize> = HashSet::from_iter(outputs.iter().map(|(o, _name)| o.idx));
            if hset.len() != outputs.len() {
                return Err(DagError::InvalidOutputs);
            }
        }
        self.workflow_outputs = outputs.to_vec();
        Ok(())
    }

    pub fn set_inputs(&mut self, inputs: &[(IH, usize, &str)]) -> Result<(), DagError> {
        {
            // Validate the input indices. They must be sequential, and start from zero.
            let mut unique_indices = inputs
                .iter()
                .map(|(_input, idx, _name)| *idx)
                .collect::<Vec<_>>();
            unique_indices.sort();
            unique_indices.dedup();
            if unique_indices.iter().enumerate().any(|(i, val)| i != *val) {
                return Err(DagError::InvalidInputs);
            }
        }
        let mut input_idx = self.workflow_input_index.try_borrow_mut()?;
        let input_idx: &mut InputPropBuf<_> = &mut input_idx;
        input_idx.fill(None);
        for (input, idx, name) in inputs.iter() {
            input_idx[*input] = Some((*idx, name.to_string()));
        }
        Ok(())
    }

    pub fn add_function(
        &mut self,
        info: FuncInfo,
        input_handles: &mut [IH],
        output_handles: &mut [OH],
    ) -> Result<NH, DagError> {
        let n = self.graph.push_node(input_handles, output_handles)?;
        {
            let mut node_infos = self.node_infos.try_borrow_mut()?;
            node_infos[n] = NodeInfo::Function(info);
        }
        Ok(n)
    }

    pub fn add_constant(&mut self, data: OrcHandle) -> Result<(NH, OH), DagError> {
        let mut output_handle = OH { idx: 0 };
        let n = self
            .graph
            .push_node(&mut [], std::slice::from_mut(&mut output_handle))?;
        {
            let mut node_infos = self.node_infos.try_borrow_mut()?;
            node_infos[n] = NodeInfo::Constant {
                name: String::new(),
                data,
            };
        }
        Ok((n, output_handle))
    }

    pub fn connect(&mut self, from: OH, to: IH) -> Result<LH, DagError> {
        self.graph.push_link(from, to)
    }

    pub fn disconnect(&mut self, from: OH, to: IH) -> bool {
        // Only successfully disconnect if the given input / output pairs are actually connected.
        if let Some(src) = self.graph.input_source(to)
            && src == from
        {
            self.graph.disconnect_input(to);
            true
        } else {
            false
        }
    }

    pub fn reserve(
        &mut self,
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Result<(), DagError> {
        self.graph.reserve(n_inputs, n_outputs, n_links, n_nodes)
    }

    pub fn clear(&mut self) -> Result<(), DagError> {
        self.graph.clear()
    }

    pub fn create_input_property<T>(&mut self, default: T) -> InputProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_input_property(default)
    }

    pub fn create_output_property<T>(&mut self, default: T) -> OutputProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_output_property(default)
    }

    pub fn create_link_property<T>(&mut self, default: T) -> LinkProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_link_property(default)
    }

    pub fn create_node_property<T>(&mut self, default: T) -> NodeProperty<T>
    where
        T: Clone + 'static,
    {
        self.graph.create_node_property(default)
    }

    pub fn garbage_collection(&mut self) -> Result<(), DagError> {
        // Cache the workflow outputs in properties first. This will preserve them through the
        // shuffling that happens in garbage collection.
        let mut output_prop: OutputProperty<Option<(usize, String)>> =
            self.create_output_property(None);
        {
            let mut temp = output_prop.try_borrow_mut()?;
            for (i, (output, name)) in self.workflow_outputs.drain(..).enumerate() {
                temp[output] = Some((i, name));
            }
        }
        self.graph.garbage_collection()?;
        // Repopulate the outputs again.
        let output_prop = output_prop.take()?;
        let mut temp = output_prop
            .into_iter()
            .enumerate()
            .filter_map(|(handle_idx, prop)| {
                prop.map(|(order, name)| (order, OH { idx: handle_idx }, name))
            })
            .collect::<Vec<_>>();
        temp.sort_by_key(|(a, ..)| *a);
        self.workflow_outputs
            .extend(temp.into_iter().map(|(_, handle, name)| (handle, name)));
        Ok(())
    }

    pub fn duplicate_node(&mut self, old: NH) -> Result<NH, DagError> {
        let src_inputs = self.graph.node_inputs(old).collect::<Box<[_]>>();
        let mut input_handles = vec![IH::default(); src_inputs.len()];
        let src_outputs = self.graph.node_outputs(old).collect::<Box<[_]>>();
        let mut output_handles = vec![OH::default(); src_outputs.len()];
        let new = self
            .graph
            .push_node(&mut input_handles, &mut output_handles)?;
        self.graph.node_props.copy(old, new)?;
        for (&new, &old) in input_handles.iter().zip(src_inputs.iter()) {
            self.graph.input_props.copy(old, new)?;
        }
        for (&new, &old) in output_handles.iter().zip(src_outputs.iter()) {
            self.graph.output_props.copy(old, new)?;
        }
        Ok(new)
    }

    pub fn get_terminal_outputs(&self) -> impl Iterator<Item = OH> {
        (0usize..self.graph.outputs.len()).filter_map(|i| match self.graph.outputs[i].link {
            Some(_) => None,
            None => Some(OH { idx: i }),
        })
    }

    /// This will run the DAG, and return an iterator over the required outputs. This is super
    /// sketchy, and sub-optimal. Probably doesn't meet the performance and memory requirements of
    /// any production quality host program. This is good enough for now, for testing as I continue
    /// to develop this.
    pub fn run(
        &self,
        inputs: &[OrcHandleBorrowed<'_>],
        outputs: &mut [OrcHandle],
        clone_fn: &dyn Fn(OrcHandleBorrowed) -> Result<OrcHandle, crate::Error>,
        handle_counter: &AtomicU64,
    ) -> Result<(), DagError> {
        // Make sure the number of outputs is what we're expecting. Inputs is a bit more flexible,
        // we don't have to check it, because there is a chance some of the functions in this
        // workflow can run without some inputs. But the outputs are not flexible.
        if outputs.len() != self.workflow_outputs.len() {
            return Err(DagError::MismatchedArrayLengths(
                outputs.len(),
                self.workflow_outputs.len(),
            ));
        }
        // For now, we're just doing a simple depth-first Euler tour of the graph, and running the
        // functions that need to be run. We can do all sorts of fancy things, like running
        // independent arms of the DAG on different threads concurrently, doing something equivalent
        // to register allocation that finds the minimal number of threads required to run the DAG
        // with maximal parallelism, etc. But those are not interesting problems to solve at this
        // time. I will revisit those topics later when they become interesting.
        let mut stack = Vec::<(NH, bool)>::new();
        stack.extend(
            self.workflow_outputs
                .iter()
                .map(|(o, _name)| (self.graph.outputs[o.idx].node, false)),
        );
        // Allocating temporary vectors like this is not idea. A host application will likely want
        // thos to run more optimally, with cached resources. But that is not an interesting problem
        // to solve at this time. I will come back to this later.
        let mut finished = vec![false; self.graph.nodes.len()].into_boxed_slice();
        let mut on_current_path = vec![false; self.graph.nodes.len()].into_boxed_slice();
        let mut computed_outputs: Box<[OrcHandle]> = (0usize..self.graph.outputs.len())
            .map(|_| OrcHandle::default())
            .collect();
        let empty_input: OrcHandle = OrcHandle::default();
        let mut temp_outputs = Vec::<OrcHandle>::new();
        let mut temp_output_handles = Vec::<OH>::new();
        // Borrow the node infos for the duration of below eval-loop.
        let node_infos = self.node_infos.try_borrow()?;
        let workflow_input_index = self.workflow_input_index.try_borrow()?;
        // Walk the graph and evaluate functions.
        while let Some((node, visited_children)) = stack.pop() {
            if finished[node.idx] {
                continue;
            }
            if visited_children {
                // We want to run this block. So we will first gather its inputs into a local buffer.
                let temp_inputs = self
                    .graph
                    .node_inputs(node)
                    .map(|input| match self.graph.input_source(input) {
                        // If it is a constant, we just borrow it's value. Otherwise we get the value associated with the upstream output.
                        Some(source) => match &node_infos[self.graph.outputs[source.idx].node] {
                            NodeInfo::Constant { name: _name, data } => Ok(data.borrowed()),
                            NodeInfo::Function(_) | NodeInfo::NestedCall { .. } => {
                                Ok(computed_outputs[source.idx].borrowed())
                            }
                        },
                        None => match &workflow_input_index[input] {
                            Some((index, _name)) if *index < inputs.len() => {
                                Ok(inputs[*index].clone())
                            }
                            _ => Ok(empty_input.borrowed()),
                        },
                    })
                    .collect::<Result<Box<[_]>, DagError>>()?;
                temp_output_handles.clear();
                temp_output_handles.extend(self.graph.node_outputs(node));
                temp_outputs.clear(); // Should have moved the previous outputs out of here anyways,
                // ========================================================================================
                // but just in case.  We're naively just incrementing the handle counter, and
                // assinging the handle values. This might be problematic in the long run, but not
                // sure. Even if it is problematic, not a big deal to fix. A host application
                // maintain its own handle-poop / handle-arena to reuse freed up handles.
                temp_outputs.resize_with(temp_output_handles.len(), || OrcHandle {
                    handle: handle_counter.fetch_add(1, Ordering::Relaxed),
                    ..Default::default()
                });
                debug_assert_eq!(temp_outputs.len(), temp_output_handles.len());
                // Run this node.
                match &node_infos[node] {
                    NodeInfo::Constant { .. } => {} // Do nothing.
                    NodeInfo::Function(func_info) => match func_info.func {
                        Some(func) => unsafe {
                            (func)(
                                node.idx as u64,
                                temp_inputs.as_ptr().cast(),
                                temp_inputs.len() as u64,
                                temp_outputs.as_mut_ptr(),
                                temp_outputs.len() as u64,
                            );
                        },
                        None => return Err(DagError::InvalidFunction),
                    },
                    NodeInfo::NestedCall { workflow_name } => {
                        match self.nested_workflows.get(workflow_name) {
                            Some(workflow) => {
                                workflow.run(
                                    &temp_inputs,
                                    &mut temp_outputs,
                                    clone_fn,
                                    handle_counter,
                                )?;
                            }
                            None => return Err(DagError::InvalidFunction),
                        }
                    }
                };
                // Move the outputs into the outer buffer for later.
                for (o, val) in temp_output_handles.drain(..).zip(temp_outputs.drain(..)) {
                    computed_outputs[o.idx] = val;
                }
                // Mark this done.
                finished[node.idx] = true;
                on_current_path[node.idx] = false;
            } else {
                if on_current_path[node.idx] {
                    return Err(DagError::CycleDetected);
                }
                // Because we're doing an Euler tour, and we're visiting this node for the first
                // time before visiting it's children, we push the node again before pushing its
                // children.
                stack.push((node, true));
                on_current_path[node.idx] = true;
                stack.extend(self.graph.node_inputs(node).filter_map(|input| {
                    self.graph
                        .input_source(input)
                        .map(|src| (self.graph.outputs[src.idx].node, false))
                }));
            }
        }
        // Now copy the final outputs of the workflow.
        for ((src, _name), dst) in self.workflow_outputs.iter().zip(outputs.iter_mut()) {
            *dst = match &node_infos[self.graph.outputs[src.idx].node] {
                NodeInfo::Constant { name: _, data } => clone_fn(data.borrowed())?,
                NodeInfo::Function(_) | NodeInfo::NestedCall { .. } => {
                    std::mem::take(&mut computed_outputs[src.idx])
                }
            };
        }
        Ok(())
    }

    pub fn set_input_label(&mut self, input: IH, name: String) -> Result<(), DagError> {
        let mut labels = self.input_labels.try_borrow_mut()?;
        labels[input] = name;
        Ok(())
    }

    pub fn set_output_label(&mut self, output: OH, name: String) -> Result<(), DagError> {
        let mut labels = self.output_labels.try_borrow_mut()?;
        labels[output] = name;
        Ok(())
    }

    pub fn input_label(&self, input: IH) -> Result<String, DagError> {
        let labels = self.input_labels.try_borrow()?;
        Ok(labels[input].clone())
    }

    pub fn output_label(&self, output: OH) -> Result<String, DagError> {
        let labels = self.output_labels.try_borrow()?;
        Ok(labels[output].clone())
    }

    pub fn set_node_comment(&mut self, node: NH, comment: &str) -> Result<(), DagError> {
        let mut comments = self.node_comments.try_borrow_mut()?;
        comments[node] = comment.to_string();
        Ok(())
    }

    pub fn append_node_comment(&mut self, node: NH, comment: &str) -> Result<(), DagError> {
        let mut comments = self.node_comments.try_borrow_mut()?;
        let dst = &mut comments[node];
        if !dst.is_empty() {
            dst.push('\n');
        }
        dst.push_str(comment);
        Ok(())
    }

    pub fn node_comment(&self, node: NH) -> Result<String, DagError> {
        let comments = self.node_comments.try_borrow()?;
        Ok(comments[node].clone())
    }

    /// Returns the workflow inputs as (IH, index, name) triples.
    pub fn workflow_inputs(&self) -> Result<Vec<(IH, usize, String)>, DagError> {
        let idx = self.workflow_input_index.try_borrow()?;
        let mut result = Vec::new();
        for i in 0..self.graph.inputs.len() {
            let ih = IH { idx: i };
            if let Some((index, name)) = &idx[ih] {
                result.push((ih, *index, name.clone()));
            }
        }
        Ok(result)
    }

    /// Returns the workflow outputs as (OH, name) pairs, in order.
    pub fn workflow_outputs(&self) -> &[(OH, String)] {
        &self.workflow_outputs
    }

    pub fn num_total_inputs(&self) -> usize {
        self.graph.inputs.len()
    }

    pub fn num_total_outputs(&self) -> usize {
        self.graph.outputs.len()
    }

    pub fn num_nodes(&self) -> usize {
        self.graph.nodes.len()
    }

    pub fn num_links(&self) -> usize {
        self.graph.links.len()
    }
}

#[cfg(test)]
mod test {
    use crate::{FuncInfo, IH, OH, Workflow, dag::Handle};

    fn make_func_info(name: &str) -> FuncInfo {
        FuncInfo {
            name: name.to_string(),
            desc: String::new(),
            n_inputs: Some(0usize),  // good enough for these tests.
            n_outputs: Some(0usize), // good enough for these tests.
            func: None,
        }
    }

    #[test]
    fn t_workflow_add_node() {
        let mut w = Workflow::default();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        let n = w
            .add_function(make_func_info("add"), &mut ins, &mut outs)
            .unwrap();
        let info = w.node_infos.get(n).unwrap();
        assert_eq!(info.name(), "add");
    }

    #[test]
    fn t_workflow_connect() {
        let mut w = Workflow::default();
        let mut a_out = [OH::default()];
        let _na = w
            .add_function(make_func_info("source"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let _nb = w
            .add_function(make_func_info("sink"), &mut b_in, &mut [])
            .unwrap();
        let lh = w.connect(a_out[0], b_in[0]).unwrap();
        assert_eq!(w.graph.links[lh.index()].start, a_out[0]);
        assert_eq!(w.graph.links[lh.index()].end, b_in[0]);
    }

    #[test]
    fn t_workflow_duplicate_node() {
        let mut w = Workflow::default();
        let mut input_labels = w.create_input_property("default_in".to_string());
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 1];
        let orig = w
            .add_function(make_func_info("mul"), &mut ins, &mut outs)
            .unwrap();
        input_labels.set(ins[0], "x".into()).unwrap();
        input_labels.set(ins[1], "y".into()).unwrap();
        let dup = w.duplicate_node(orig).unwrap();
        assert_ne!(orig, dup);
        // Duplicated node has same func info.
        let orig_info = w.node_infos.get_cloned(orig).unwrap();
        let dup_info = w.node_infos.get_cloned(dup).unwrap();
        assert_eq!(orig_info.name(), dup_info.name());
        // Duplicated node has same number of inputs/outputs.
        assert_eq!(w.graph.node_inputs(dup).count(), 2);
        assert_eq!(w.graph.node_outputs(dup).count(), 1);
        // Input properties are copied.
        let mut dup_ins = w.graph.node_inputs(dup);
        assert_eq!(*input_labels.get(dup_ins.next().unwrap()).unwrap(), "x");
        assert_eq!(*input_labels.get(dup_ins.next().unwrap()).unwrap(), "y");
        assert!(dup_ins.next().is_none());
        // Duplicated node is disconnected.
        for ih in w.graph.node_inputs(dup) {
            assert!(w.graph.inputs[ih.index()].link.is_none());
        }
    }

    #[test]
    fn t_workflow_diamond_topology() {
        // A -> B, A -> C, B -> D, C -> D
        let mut w = Workflow::default();
        let mut a_out = [OH::default(); 2];
        let na = w
            .add_function(make_func_info("A"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let mut b_out = [OH::default()];
        let nb = w
            .add_function(make_func_info("B"), &mut b_in, &mut b_out)
            .unwrap();
        let mut c_in = [IH::default()];
        let mut c_out = [OH::default()];
        let nc = w
            .add_function(make_func_info("C"), &mut c_in, &mut c_out)
            .unwrap();
        let mut d_in = [IH::default(); 2];
        let nd = w
            .add_function(make_func_info("D"), &mut d_in, &mut [])
            .unwrap();

        w.connect(a_out[0], b_in[0]).unwrap();
        w.connect(a_out[1], c_in[0]).unwrap();
        w.connect(b_out[0], d_in[0]).unwrap();
        w.connect(c_out[0], d_in[1]).unwrap();

        // Verify fan-out from A.
        assert_eq!(w.graph.node_outputs(na).count(), 2);
        // Verify D has 2 inputs, both linked.
        assert_eq!(w.graph.node_inputs(nd).count(), 2);
        for ih in w.graph.node_inputs(nd) {
            assert!(w.graph.inputs[ih.index()].link.is_some());
        }
        // Verify B and C each have 1 input, 1 output.
        assert_eq!(w.graph.node_inputs(nb).count(), 1);
        assert_eq!(w.graph.node_outputs(nb).count(), 1);
        assert_eq!(w.graph.node_inputs(nc).count(), 1);
        assert_eq!(w.graph.node_outputs(nc).count(), 1);
    }

    #[test]
    fn t_workflow_disconnect() {
        let mut w = Workflow::default();
        let mut a_out = [OH::default()];
        let _na = w
            .add_function(make_func_info("src"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let _nb = w
            .add_function(make_func_info("dst"), &mut b_in, &mut [])
            .unwrap();
        let lh = w.connect(a_out[0], b_in[0]).unwrap();
        // Disconnect the correct pair.
        assert!(w.disconnect(a_out[0], b_in[0]));
        assert!(w.graph.links[lh.index()].deleted);
        assert_eq!(w.graph.inputs[b_in[0].index()].link, None);
        assert_eq!(w.graph.outputs[a_out[0].index()].link, None);
    }

    #[test]
    fn t_workflow_disconnect_wrong_pair() {
        let mut w = Workflow::default();
        let mut a_out = [OH::default()];
        let _na = w
            .add_function(make_func_info("A"), &mut [], &mut a_out)
            .unwrap();
        let mut b_out = [OH::default()];
        let _nb = w
            .add_function(make_func_info("B"), &mut [], &mut b_out)
            .unwrap();
        let mut c_in = [IH::default()];
        let _nc = w
            .add_function(make_func_info("C"), &mut c_in, &mut [])
            .unwrap();
        // Connect A -> C.
        let lh = w.connect(a_out[0], c_in[0]).unwrap();
        // Try to disconnect B -> C (wrong output). Should fail.
        assert!(!w.disconnect(b_out[0], c_in[0]));
        // Original link is still intact.
        assert!(!w.graph.links[lh.index()].deleted);
        assert_eq!(w.graph.inputs[c_in[0].index()].link, Some(lh));
    }

    #[test]
    fn t_workflow_disconnect_unconnected() {
        let mut w = Workflow::default();
        let mut a_out = [OH::default()];
        let _na = w
            .add_function(make_func_info("A"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default()];
        let _nb = w
            .add_function(make_func_info("B"), &mut b_in, &mut [])
            .unwrap();
        // No connection exists. Disconnect should return false.
        assert!(!w.disconnect(a_out[0], b_in[0]));
    }

    #[test]
    fn t_workflow_clear() {
        let mut w = Workflow::default();
        let mut outs = [OH::default()];
        let _n = w
            .add_function(make_func_info("x"), &mut [], &mut outs)
            .unwrap();
        w.clear().unwrap();
        assert!(w.graph.nodes.is_empty());
        assert!(w.graph.inputs.is_empty());
        assert!(w.graph.outputs.is_empty());
        assert!(w.graph.links.is_empty());
    }

    #[test]
    fn t_workflow_with_custom_properties() {
        let mut w = Workflow::default();
        let mut dirty = w.create_node_property(false);
        let mut input_vals = w.create_input_property(0.0f64);

        let mut a_out = [OH::default()];
        let na = w
            .add_function(make_func_info("source"), &mut [], &mut a_out)
            .unwrap();
        let mut b_in = [IH::default(); 2];
        let nb = w
            .add_function(make_func_info("add"), &mut b_in, &mut [])
            .unwrap();

        dirty.set(na, true).unwrap();
        input_vals.set(b_in[0], 3.1234).unwrap();
        input_vals.set(b_in[1], 2.72).unwrap();

        assert!(*dirty.get(na).unwrap());
        assert!(!(*dirty.get(nb).unwrap())); // default
        assert_eq!(*input_vals.get(b_in[0]).unwrap(), 3.1234);
        assert_eq!(*input_vals.get(b_in[1]).unwrap(), 2.72);
    }
}
