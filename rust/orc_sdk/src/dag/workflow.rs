use super::{
    DagError, Graph, IH, InputProperty, LH, LinkProperty, NH, NodeProperty, OH, OutputProperty,
    Property,
};
use crate::{FuncInfo, OrcHandle};

pub enum NodeInfo {
    Parameter { name: String },
    Constant { name: String, data: OrcHandle },
    Function(FuncInfo),
}

impl Clone for NodeInfo {
    fn clone(&self) -> Self {
        match self {
            Self::Parameter { name } => Self::Parameter { name: name.clone() },
            // The host application is responsible for allocating the variable data manually.
            Self::Constant { name, data: _data } => Self::Constant {
                name: format!("{name}_copy"),
                data: OrcHandle::default(),
            },
            Self::Function(arg0) => Self::Function(arg0.clone()),
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
            NodeInfo::Parameter { name } => name,
            NodeInfo::Constant { name, .. } => name,
            NodeInfo::Function(func_info) => &func_info.name,
        }
    }
}

pub struct Workflow {
    graph: Graph,
    node_infos: NodeProperty<NodeInfo>,
}

impl Default for Workflow {
    fn default() -> Self {
        let mut graph = Graph::default();
        let node_infos = graph.create_node_property(NodeInfo::default());
        Self { graph, node_infos }
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
        Workflow { graph, node_infos }
    }

    pub fn add_function(
        &mut self,
        info: FuncInfo,
        input_handles: &mut [IH],
        output_handles: &mut [OH],
    ) -> Result<NH, DagError> {
        let n = self.graph.push_node(input_handles, output_handles)?;
        {
            let mut node_infos = self
                .node_infos
                .try_borrow_mut()
                .map_err(|_e| DagError::BorrowedPropertyAccess)?;
            node_infos[n] = NodeInfo::Function(info);
        }
        Ok(n)
    }

    pub fn add_constant(
        &mut self,
        data: OrcHandle,
        output_handle: &mut OH,
    ) -> Result<NH, DagError> {
        let n = self
            .graph
            .push_node(&mut [], std::slice::from_mut(output_handle))?;
        {
            let mut node_infos = self
                .node_infos
                .try_borrow_mut()
                .map_err(|_e| DagError::BorrowedPropertyAccess)?;
            node_infos[n] = NodeInfo::Constant {
                name: String::new(),
                data,
            };
        }
        Ok(n)
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
        self.graph.garbage_collection()
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
