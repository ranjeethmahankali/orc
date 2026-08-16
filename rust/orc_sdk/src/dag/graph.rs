use super::{
    DagError, InputProperty, LinkProperty, NodeProperty, OutputProperty, PropertyContainer,
};

/// All elements of the graph implement this trait. They are identified by their
/// index.
pub trait Handle: From<usize> + Copy + Clone + 'static {
    /// The index of the element.
    fn index(&self) -> usize;
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct IH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct OH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct LH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub struct NH {
    idx: usize,
}

impl From<usize> for IH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for IH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for OH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for OH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for LH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for LH {
    fn index(&self) -> usize {
        self.idx
    }
}

impl From<usize> for NH {
    fn from(idx: usize) -> Self {
        Self { idx }
    }
}

impl Handle for NH {
    fn index(&self) -> usize {
        self.idx
    }
}

#[derive(Clone)]
pub(crate) struct Input {
    pub(crate) node: NH,
    pub(crate) link: Option<LH>,
    pub(crate) prev: Option<IH>,
    pub(crate) next: Option<IH>,
    pub(crate) deleted: bool,
}

#[derive(Clone)]
pub(crate) struct Output {
    pub(crate) node: NH,
    pub(crate) link: Option<LH>,
    pub(crate) prev: Option<OH>,
    pub(crate) next: Option<OH>,
    pub(crate) deleted: bool,
}

#[derive(Clone)]
pub(crate) struct Link {
    pub(crate) start: OH,
    pub(crate) end: IH,
    pub(crate) prev: Option<LH>,
    pub(crate) next: Option<LH>,
    pub(crate) deleted: bool,
}

#[derive(Clone)]
pub(crate) struct Node {
    pub(crate) input: Option<IH>,
    pub(crate) output: Option<OH>,
    pub(crate) deleted: bool,
}

#[derive(Default)]
pub struct Graph {
    pub(crate) inputs: Vec<Input>,
    pub(crate) outputs: Vec<Output>,
    pub(crate) links: Vec<Link>,
    pub(crate) nodes: Vec<Node>,
    // Property containers.
    pub(crate) node_props: PropertyContainer<NH>,
    pub(crate) link_props: PropertyContainer<LH>,
    pub(crate) input_props: PropertyContainer<IH>,
    pub(crate) output_props: PropertyContainer<OH>,
}

impl Graph {
    pub fn with_capacity(
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Self {
        Graph {
            inputs: Vec::with_capacity(n_inputs),
            outputs: Vec::with_capacity(n_outputs),
            links: Vec::with_capacity(n_links),
            nodes: Vec::with_capacity(n_nodes),
            node_props: Default::default(),
            link_props: Default::default(),
            input_props: Default::default(),
            output_props: Default::default(),
        }
    }

    pub fn push_node(
        &mut self,
        input_handles: &mut [IH],
        output_handles: &mut [OH],
    ) -> Result<NH, DagError> {
        let nh = NH {
            idx: self.nodes.len(),
        };
        // Push the input pins.
        for input_handle in input_handles.iter_mut() {
            input_handle.idx = self.inputs.len();
            self.push_input_impl(Input {
                node: nh,
                link: None,
                prev: None,
                next: None,
                deleted: false,
            })?;
        }
        // Link the consecutive input pins together.
        for [prev, next] in input_handles.array_windows::<2>() {
            self.link_consecutive_inputs(*prev, *next);
        }
        // Push output pins.
        for output_handle in output_handles.iter_mut() {
            output_handle.idx = self.outputs.len();
            self.push_output_impl(Output {
                node: nh,
                link: None,
                prev: None,
                next: None,
                deleted: false,
            })?;
        }
        // Link the consecutive output pins together.
        for [prev, next] in output_handles.array_windows::<2>() {
            self.link_consecutive_outputs(*prev, *next);
        }
        self.push_node_impl(Node {
            input: input_handles.first().copied(),
            output: output_handles.first().copied(),
            deleted: false,
        })
        .map(|()| nh)
    }

    fn push_input_impl(&mut self, input: Input) -> Result<(), DagError> {
        self.inputs.push(input);
        self.input_props.push_value()
    }

    fn push_output_impl(&mut self, output: Output) -> Result<(), DagError> {
        self.outputs.push(output);
        self.output_props.push_value()
    }

    fn push_node_impl(&mut self, node: Node) -> Result<(), DagError> {
        self.nodes.push(node);
        self.node_props.push_value()
    }

    pub fn push_link(&mut self, from: OH, to: IH) -> Result<LH, DagError> {
        // Disconnect any existing link on the input first, before walking the
        // output's linked list. Otherwise, if the old link belongs to the same
        // output, the tail pointer computed below would go stale.
        self.disconnect_input(to);
        let mut last: Option<LH> = None;
        {
            let mut tail = &self.outputs[from.idx].link;
            debug_assert!(
                match tail {
                    Some(li) => self.links[li.idx].prev.is_none(),
                    None => true,
                },
                "The first link should not have a previous link. The topology is broken."
            );
            while let Some(li) = *tail {
                last = Some(li);
                tail = &self.links[li.idx].next;
            }
        }
        // Push the link first so it exists in the vec before we reference it.
        let lh = LH {
            idx: self.links.len(),
        };
        self.push_link_impl(Link {
            start: from,
            end: to,
            prev: last,
            next: None,
            deleted: false,
        })?;
        self.inputs[to.idx].link = Some(lh);
        // Append to the output's linked list.
        match last {
            Some(p) => self.links[p.idx].next = Some(lh),
            None => self.outputs[from.idx].link = Some(lh),
        }
        Ok(lh)
    }

    pub fn disconnect_input(&mut self, input: IH) {
        if let Some(old) = self.inputs[input.idx].link.take() {
            self.links[old.idx].deleted = true;
            // Remove references to the deleted link.
            let prev = self.links[old.idx].prev.take();
            let next = self.links[old.idx].next.take();
            let src = self.links[old.idx].start;
            if self.outputs[src.idx].link == Some(old) {
                self.outputs[src.idx].link = next;
            }
            if let Some(prev) = prev {
                self.links[prev.idx].next = next;
            }
            if let Some(next) = next {
                self.links[next.idx].prev = prev;
            }
        }
    }

    fn push_link_impl(&mut self, link: Link) -> Result<(), DagError> {
        self.links.push(link);
        self.link_props.push_value()
    }

    pub fn create_input_property<T>(&mut self, default: T) -> InputProperty<T>
    where
        T: Clone + 'static,
    {
        InputProperty::new(&mut self.input_props, default)
    }

    pub fn create_output_property<T>(&mut self, default: T) -> OutputProperty<T>
    where
        T: Clone + 'static,
    {
        OutputProperty::new(&mut self.output_props, default)
    }

    pub fn create_link_property<T>(&mut self, default: T) -> LinkProperty<T>
    where
        T: Clone + 'static,
    {
        LinkProperty::new(&mut self.link_props, default)
    }

    pub fn create_node_property<T>(&mut self, default: T) -> NodeProperty<T>
    where
        T: Clone + 'static,
    {
        NodeProperty::new(&mut self.node_props, default)
    }

    fn link_consecutive_inputs(&mut self, prev: IH, next: IH) {
        self.inputs[prev.idx].next = Some(next);
        self.inputs[next.idx].prev = Some(prev);
    }

    fn link_consecutive_outputs(&mut self, prev: OH, next: OH) {
        self.outputs[prev.idx].next = Some(next);
        self.outputs[next.idx].prev = Some(prev);
    }

    pub fn reserve(
        &mut self,
        n_inputs: usize,
        n_outputs: usize,
        n_links: usize,
        n_nodes: usize,
    ) -> Result<(), DagError> {
        self.inputs.reserve(n_inputs);
        self.outputs.reserve(n_outputs);
        self.links.reserve(n_links);
        self.nodes.reserve(n_nodes);
        // Also reserve the property containers.
        self.input_props.reserve(n_inputs)?;
        self.output_props.reserve(n_outputs)?;
        self.link_props.reserve(n_links)?;
        self.node_props.reserve(n_nodes)?;
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), DagError> {
        self.inputs.clear();
        self.outputs.clear();
        self.links.clear();
        self.nodes.clear();
        // Also clear the property containers.
        self.input_props.clear()?;
        self.output_props.clear()?;
        self.link_props.clear()?;
        self.node_props.clear()?;
        Ok(())
    }

    pub fn garbage_collection(&mut self) -> Result<(), DagError> {
        let mut imap: Vec<IH> = (0usize..self.inputs.len()).map(|i| IH { idx: i }).collect();
        let mut omap: Vec<OH> = (0usize..self.outputs.len())
            .map(|i| OH { idx: i })
            .collect();
        let mut lmap: Vec<LH> = (0usize..self.links.len()).map(|i| LH { idx: i }).collect();
        let mut nmap: Vec<NH> = (0usize..self.nodes.len()).map(|i| NH { idx: i }).collect();
        // Clean up inputs.
        if !self.inputs.is_empty() {
            let mut left = 0usize;
            let mut right = self.inputs.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.inputs[left].deleted && left < right {
                    left += 1;
                }
                while self.inputs[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.inputs[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.inputs.swap(left, right);
                self.input_props.swap(left, right)?;
                imap.swap(left, right);
            };
            self.inputs.truncate(newlen);
            self.input_props.resize(newlen)?;
        }
        // Clean up outputs.
        if !self.outputs.is_empty() {
            let mut left = 0usize;
            let mut right = self.outputs.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.outputs[left].deleted && left < right {
                    left += 1;
                }
                while self.outputs[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.outputs[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.outputs.swap(left, right);
                self.output_props.swap(left, right)?;
                omap.swap(left, right);
            };
            self.outputs.truncate(newlen);
            self.output_props.resize(newlen)?;
        }
        // Clean up links.
        if !self.links.is_empty() {
            let mut left = 0usize;
            let mut right = self.links.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.links[left].deleted && left < right {
                    left += 1;
                }
                while self.links[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.links[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.links.swap(left, right);
                self.link_props.swap(left, right)?;
                lmap.swap(left, right);
            };
            self.links.truncate(newlen);
            self.link_props.resize(newlen)?;
        }
        // Clean up nodes.
        if !self.nodes.is_empty() {
            let mut left = 0usize;
            let mut right = self.nodes.len() - 1;
            let newlen = loop {
                // Find first deleted and last un-deleted.
                while !self.nodes[left].deleted && left < right {
                    left += 1;
                }
                while self.nodes[right].deleted && left < right {
                    right -= 1;
                }
                if left >= right {
                    break left + if self.nodes[left].deleted { 0 } else { 1 };
                }
                // Swap the deleted and the undeleted.
                self.nodes.swap(left, right);
                self.node_props.swap(left, right)?;
                nmap.swap(left, right);
            };
            self.nodes.truncate(newlen);
            self.node_props.resize(newlen)?;
        }
        // Update inputs.
        for input in self.inputs.iter_mut() {
            debug_assert!(!input.deleted, "Garbage collection isn't working properly");
            input.node = nmap[input.node.idx];
            if let Some(l) = &mut input.link {
                *l = lmap[l.idx]
            }
            if let Some(p) = &mut input.prev {
                *p = imap[p.idx]
            }
            if let Some(n) = &mut input.next {
                *n = imap[n.idx]
            }
        }
        // Update Outputs.
        for output in self.outputs.iter_mut() {
            debug_assert!(!output.deleted, "Garbage collection isn't working properly");
            output.node = nmap[output.node.idx];
            if let Some(l) = &mut output.link {
                *l = lmap[l.idx];
            }
            if let Some(p) = &mut output.prev {
                *p = omap[p.idx]
            }
            if let Some(n) = &mut output.next {
                *n = omap[n.idx]
            }
        }
        // Update links.
        for link in self.links.iter_mut() {
            debug_assert!(!link.deleted, "Garbage collection isn't working properly");
            link.start = omap[link.start.idx];
            link.end = imap[link.end.idx];
            if let Some(l) = &mut link.prev {
                *l = lmap[l.idx];
            }
            if let Some(l) = &mut link.next {
                *l = lmap[l.idx];
            }
        }
        // Update nodes.
        for node in self.nodes.iter_mut() {
            debug_assert!(!node.deleted, "Garbage collection isn't working properly");
            if let Some(i) = &mut node.input {
                *i = imap[i.idx];
            }
            if let Some(o) = &mut node.output {
                *o = omap[o.idx];
            }
        }
        // Clean up properties.
        self.input_props.garbage_collection();
        self.output_props.garbage_collection();
        self.link_props.garbage_collection();
        self.node_props.garbage_collection();
        Ok(())
    }

    pub fn node_inputs(&self, n: NH) -> impl Iterator<Item = IH> {
        std::iter::successors(self.nodes[n.idx].input, |i| self.inputs[i.idx].next)
    }

    pub fn node_outputs(&self, n: NH) -> impl Iterator<Item = OH> {
        std::iter::successors(self.nodes[n.idx].output, |o| self.outputs[o.idx].next)
    }

    pub fn input_source(&self, i: IH) -> Option<OH> {
        self.inputs[i.idx].link.map(|l| self.links[l.idx].start)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::dag::test::chain_graph;

    #[test]
    fn t_push_node_assigns_handles() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 2];
        let mut outs = [OH::default(); 3];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert_eq!(n.idx, 0);
        assert_eq!(ins[0].idx, 0);
        assert_eq!(ins[1].idx, 1);
        assert_eq!(outs[0].idx, 0);
        assert_eq!(outs[1].idx, 1);
        assert_eq!(outs[2].idx, 2);
    }

    #[test]
    fn t_node_inputs_outputs_iteration() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 3];
        let mut outs = [OH::default(); 2];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert!(g.node_inputs(n).eq(ins));
        assert!(g.node_outputs(n).eq(outs));
    }

    #[test]
    fn t_num_node_inputs_outputs() {
        let mut g = Graph::default();
        let mut ins = [IH::default(); 4];
        let mut outs = [OH::default(); 1];
        let n = g.push_node(&mut ins, &mut outs).unwrap();
        assert_eq!(g.node_inputs(n).count(), 4);
        assert_eq!(g.node_outputs(n).count(), 1);
    }

    #[test]
    fn t_zero_inputs_zero_outputs() {
        let mut g = Graph::default();
        let n = g.push_node(&mut [], &mut []).unwrap();
        assert_eq!(g.node_inputs(n).count(), 0);
        assert_eq!(g.node_outputs(n).count(), 0);
        assert_eq!(g.node_inputs(n).count(), 0);
        assert_eq!(g.node_outputs(n).count(), 0);
    }

    #[test]
    fn t_push_link_connects() {
        let (g, _, ins, outs, links) = chain_graph();
        // A's output links to B's input.
        assert_eq!(g.links[links[0].idx].start, outs[0]);
        assert_eq!(g.links[links[0].idx].end, ins[0]);
        // B's input references the link.
        assert_eq!(g.inputs[ins[0].idx].link, Some(links[0]));
    }

    #[test]
    fn t_multiple_links_from_one_output() {
        let mut g = Graph::default();
        let mut a_out = [OH::default()];
        let _na = g.push_node(&mut [], &mut a_out).unwrap();

        let mut b_in = [IH::default()];
        let _nb = g.push_node(&mut b_in, &mut []).unwrap();

        let mut c_in = [IH::default()];
        let _nc = g.push_node(&mut c_in, &mut []).unwrap();

        let l0 = g.push_link(a_out[0], b_in[0]).unwrap();
        let l1 = g.push_link(a_out[0], c_in[0]).unwrap();

        // Output's linked list: l0 -> l1.
        assert_eq!(g.outputs[a_out[0].idx].link, Some(l0));
        assert_eq!(g.links[l0.idx].next, Some(l1));
        assert_eq!(g.links[l1.idx].prev, Some(l0));
        assert_eq!(g.links[l1.idx].next, None);
    }

    #[test]
    fn t_reconnect_input_from_different_output() {
        let mut g = Graph::default();
        let mut a_out = [OH::default()];
        let _na = g.push_node(&mut [], &mut a_out).unwrap();
        let mut b_out = [OH::default()];
        let _nb = g.push_node(&mut [], &mut b_out).unwrap();
        let mut c_in = [IH::default()];
        let _nc = g.push_node(&mut c_in, &mut []).unwrap();

        // Connect A -> C.
        let l0 = g.push_link(a_out[0], c_in[0]).unwrap();
        assert_eq!(g.inputs[c_in[0].idx].link, Some(l0));
        // Reconnect B -> C. Old link should be deleted.
        let l1 = g.push_link(b_out[0], c_in[0]).unwrap();
        assert_eq!(g.inputs[c_in[0].idx].link, Some(l1));
        assert!(g.links[l0.idx].deleted);
        // A's output has no live links.
        assert_eq!(g.outputs[a_out[0].idx].link, None);
        // B's output has the new link.
        assert_eq!(g.outputs[b_out[0].idx].link, Some(l1));
    }

    #[test]
    fn t_reconnect_input_from_same_output() {
        // Output O fans out to I1 and I2. Reconnecting O -> I2 should keep
        // the output's linked list intact (regression test for stale tail).
        let mut g = Graph::default();
        let mut o = [OH::default()];
        let _src = g.push_node(&mut [], &mut o).unwrap();
        let mut i1 = [IH::default()];
        let _n1 = g.push_node(&mut i1, &mut []).unwrap();
        let mut i2 = [IH::default()];
        let _n2 = g.push_node(&mut i2, &mut []).unwrap();

        let l0 = g.push_link(o[0], i1[0]).unwrap();
        let _l1 = g.push_link(o[0], i2[0]).unwrap();
        // Reconnect O -> I2 (same pair as _l1).
        let l2 = g.push_link(o[0], i2[0]).unwrap();
        // l0 is still live, _l1 is deleted, l2 is the new link.
        assert!(!g.links[l0.idx].deleted);
        assert!(g.links[_l1.idx].deleted);
        assert!(!g.links[l2.idx].deleted);
        // Output's linked list: l0 -> l2.
        assert_eq!(g.outputs[o[0].idx].link, Some(l0));
        assert_eq!(g.links[l0.idx].next, Some(l2));
        assert_eq!(g.links[l2.idx].prev, Some(l0));
        assert_eq!(g.links[l2.idx].next, None);
        // Both inputs are connected.
        assert_eq!(g.inputs[i1[0].idx].link, Some(l0));
        assert_eq!(g.inputs[i2[0].idx].link, Some(l2));
    }

    #[test]
    fn t_reconnect_sole_link_on_output() {
        // Output has exactly one link O -> I. Reconnecting O -> I should
        // replace it cleanly (the old link was both head and tail).
        let mut g = Graph::default();
        let mut o = [OH::default()];
        let _src = g.push_node(&mut [], &mut o).unwrap();
        let mut i = [IH::default()];
        let _dst = g.push_node(&mut i, &mut []).unwrap();

        let l0 = g.push_link(o[0], i[0]).unwrap();
        let l1 = g.push_link(o[0], i[0]).unwrap();
        assert!(g.links[l0.idx].deleted);
        assert!(!g.links[l1.idx].deleted);
        assert_eq!(g.outputs[o[0].idx].link, Some(l1));
        assert_eq!(g.links[l1.idx].prev, None);
        assert_eq!(g.links[l1.idx].next, None);
        assert_eq!(g.inputs[i[0].idx].link, Some(l1));
    }

    #[test]
    fn t_chain_structure() {
        let (g, nodes, ins, outs, _) = chain_graph();
        // A has 0 inputs, 1 output.
        assert_eq!(g.node_inputs(nodes[0]).count(), 0);
        assert_eq!(g.node_outputs(nodes[0]).count(), 1);
        // B has 1 input, 1 output.
        assert_eq!(g.node_inputs(nodes[1]).count(), 1);
        assert_eq!(g.node_outputs(nodes[1]).count(), 1);
        // C has 1 input, 0 outputs.
        assert_eq!(g.node_inputs(nodes[2]).count(), 1);
        assert_eq!(g.node_outputs(nodes[2]).count(), 0);
        // B's input belongs to node B.
        assert_eq!(g.inputs[ins[0].idx].node, nodes[1]);
        // A's output belongs to node A.
        assert_eq!(g.outputs[outs[0].idx].node, nodes[0]);
    }

    #[test]
    fn t_clear_empties_everything() {
        let (mut g, _, _, _, _) = chain_graph();
        g.clear().unwrap();
        assert!(g.inputs.is_empty());
        assert!(g.outputs.is_empty());
        assert!(g.links.is_empty());
        assert!(g.nodes.is_empty());
    }
}
