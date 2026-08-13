/// Declarative macro for composing a DAG of plugin function calls.
///
/// Takes a PluginSet, an `&AtomicU64` handle counter, and a block of statements.
/// The counter is used to assign unique handle IDs to all output handles.
/// Supports nesting single-output calls: `(add (mul a b) c)`.
///
/// ```ignore
/// use std::sync::atomic::AtomicU64;
/// let hc = AtomicU64::new(0);
/// orc_dag!(plugin_set, &hc, {
///     (let tmp (mul a b))
///     (let result (add tmp c))
///     (let (mean variance) (compute_stats data))
///     (add (mul a b) c)
/// })
/// ```
#[macro_export]
macro_rules! orc_dag {
    // Entry point: takes a PluginSet, an &AtomicU64 handle counter, and a block of statements.
    ($ps:expr, $hc:expr, { $($body:tt)* }) => {{
        #[allow(unused_variables)]
        let ps_ref_ = &$ps;
        #[allow(unused_variables)]
        let hc_ref_: &std::sync::atomic::AtomicU64 = $hc;
        orc_dag!(@stmts ps_ref_, hc_ref_, $($body)*)
    }};

    // --- Statements ---

    // let single output, more statements follow
    (@stmts $ps:ident, $hc:ident, (let $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = orc_dag!(@call1 $ps, $hc, $func, $($arg)*);
        orc_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // let multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, (let ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        orc_dag!(@call_n $ps, $hc, ($($name)+) $func, $($arg)*);
        orc_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident, $hc:ident,) => { () };

    // --- Expression: either a variable reference or a nested function call ---
    // Nested call: (func args...)
    (@expr $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $func, $($arg)*)
    };
    // Variable reference
    (@expr $ps:ident, $hc:ident, $var:ident) => {
        unsafe { $var.non_owning_clone() }
    };

    // --- Single-output function call ---
    (@call1 $ps:ident, $hc:ident, $func:ident, $($arg:tt)*) => {{
        let inputs_ = [$(orc_dag!(@expr $ps, $hc, $arg)),*];
        let mut out_: OrcHandle = OrcHandle::default();
        out_.handle = $hc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        unsafe {
            (func_.func)(0, inputs_.as_ptr(), inputs_.len() as u64, &mut out_, 1);
        }
        out_
    }};

    // --- Multi-output function call ---
    (@call_n $ps:ident, $hc:ident, ($($name:ident)+) $func:ident, $($arg:tt)*) => {
        let inputs_ = [$(orc_dag!(@expr $ps, $hc, $arg)),*];
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        const N_OUTS_: u64 = orc_dag!(@count $($name)+) as u64;
        let base_handle_ = $hc.fetch_add(N_OUTS_, std::sync::atomic::Ordering::Relaxed);
        let mut outs_: [OrcHandle; N_OUTS_ as usize] = std::array::from_fn(|i| {
            let mut h = OrcHandle::default();
            h.handle = base_handle_ + i as u64;
            h
        });
        unsafe {
            (func_.func)(0, inputs_.as_ptr(), inputs_.len() as u64, outs_.as_mut_ptr(), N_OUTS_);
        }
        let mut idx_ = 0usize;
        $(
            let $name = std::mem::take(&mut outs_[idx_]);
            idx_ += 1;
        )+
        let _ = idx_;
    };

    // --- Counting ---
    (@count $x:ident) => { 1usize };
    (@count $x:ident $($rest:ident)+) => { 1usize + orc_dag!(@count $($rest)+) };

    // --- Error messages for wrong usage ---
    ($ps:expr, { $($body:tt)* }) => {
        compile_error!("orc_dag! requires 3 arguments: orc_dag!(plugin_set, handle_counter, { ... })")
    };
    ($ps:expr) => {
        compile_error!("orc_dag! requires 3 arguments: orc_dag!(plugin_set, handle_counter, { ... })")
    };
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
struct IH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
struct OH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
struct LH {
    idx: usize,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Default)]
struct NH {
    idx: usize,
}

struct Node {
    input: Option<IH>,
    output: Option<OH>,
}

struct Input {
    node: NH,
    link: Option<LH>,
    prev: Option<IH>,
    next: Option<IH>,
}

struct Output {
    node: NH,
    link: Option<LH>,
    prev: Option<OH>,
    next: Option<OH>,
}

struct Link {
    start: OH,
    end: IH,
    prev: Option<LH>,
    next: Option<LH>,
}

pub struct Workflow {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    links: Vec<Link>,
    nodes: Vec<Node>,
}

impl Workflow {
    pub fn push_node(&mut self, input_handles: &mut [IH], output_handles: &mut [OH]) -> NH {
        let nh = NH {
            idx: self.nodes.len(),
        };
        // Push the input pins.
        for input_handle in input_handles.iter_mut() {
            input_handle.idx = self.inputs.len();
            self.inputs.push(Input {
                node: nh,
                link: None,
                prev: None,
                next: None,
            });
        }
        // Link the consecutive input pins together.
        for [prev, next] in input_handles.array_windows::<2>() {
            self.link_consecutive_inputs(*prev, *next);
        }
        // Push output pins.
        for output_handle in output_handles.iter_mut() {
            output_handle.idx = self.outputs.len();
            self.outputs.push(Output {
                node: nh,
                link: None,
                prev: None,
                next: None,
            });
        }
        // Link the consecutive output pins together.
        for [prev, next] in output_handles.array_windows::<2>() {
            self.link_consecutive_outputs(*prev, *next);
        }
        self.nodes.push(Node {
            input: input_handles.first().copied(),
            output: output_handles.first().copied(),
        });
        nh
    }

    pub fn push_link(&mut self, from: OH, to: IH) -> Option<LH> {
        // TODO: Check to see if this link creates a cycle, and return None if it does.
        // Find the tail of the output's linked list of outgoing links.
        let mut prev: Option<LH> = None;
        {
            let mut tail = &self.outputs[from.idx].link;
            debug_assert!(
                match tail {
                    Some(li) => self.links[li.idx].prev == None,
                    None => true,
                },
                "The first link should not have a previous link. The topology is broken."
            );
            while let Some(li) = *tail {
                prev = Some(li);
                tail = &self.links[li.idx].next;
            }
        }
        // Push the link first so it exists in the vec before we reference it.
        let lh = LH {
            idx: self.links.len(),
        };
        self.links.push(Link {
            start: from,
            end: to,
            prev,
            next: None,
        });
        // Connect the link to the input. Inputs only have one incoming link.
        self.inputs[to.idx].link = Some(lh);
        // Append to the output's linked list.
        match prev {
            Some(p) => self.links[p.idx].next = Some(lh),
            None => self.outputs[from.idx].link = Some(lh),
        }
        Some(lh)
    }

    fn link_consecutive_inputs(&mut self, prev: IH, next: IH) {
        self.inputs[prev.idx].next = Some(next);
        self.inputs[next.idx].prev = Some(prev);
    }

    fn link_consecutive_outputs(&mut self, prev: OH, next: OH) {
        self.outputs[prev.idx].next = Some(next);
        self.outputs[next.idx].prev = Some(prev);
    }
}
