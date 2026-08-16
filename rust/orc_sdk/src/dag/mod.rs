mod graph;
pub use graph::{Graph, Handle, IH, LH, NH, OH};

mod property;
pub use property::{
    InputPropBuf, InputProperty, LinkPropBuf, LinkProperty, NodePropBuf, NodeProperty,
    OutputPropBuf, OutputProperty, Property, PropertyContainer,
};

mod workflow;
pub use workflow::{DagOutputData, NodeInfo, Workflow};

/// Declarative macro for composing a DAG of plugin function calls.
///
/// Takes a PluginSet, an `&AtomicU64` handle counter, a `&DeckRegistry`, and a
/// block of statements. The counter is used to assign unique handle IDs to all
/// output handles. The registry is used to allocate owning handles for `const`
/// expressions.
///
/// ```ignore
/// use std::sync::atomic::AtomicU64;
/// let hc = AtomicU64::new(0);
/// orc_inline_dag!(plugin_set, &hc, &registry, {
///     (let a (const [[1.0, 2.0], [3.0]]))
///     (let b (const 10.0f64))
///     (let tmp (mul a b))
///     (let result (add tmp c))
///     (let (mean variance) (compute_stats data))
///     (add (mul a b) c)
/// })
/// ```
#[macro_export]
macro_rules! orc_inline_dag {
    // Entry point: takes a PluginSet, an &AtomicU64 handle counter, a &DeckRegistry, and a block.
    ($ps:expr, $hc:expr, $reg:expr, { $($body:tt)* }) => {
        (|| -> Result<_, $crate::Error> {
            #[allow(unused_variables)]
            let ps_ref_ = &$ps;
            #[allow(unused_variables)]
            let hc_ref_: &std::sync::atomic::AtomicU64 = $hc;
            #[allow(unused_variables)]
            let reg_ref_: &$crate::DeckRegistry = $reg;
            orc_inline_dag!(@stmts ps_ref_, hc_ref_, reg_ref_, $($body)*)
        })()
    };

    // --- Statements ---

    // let const with list literal: (let x (const [1, 2, 3]))
    (@stmts $ps:ident, $hc:ident, $reg:ident, (let $name:ident (const [$($tt:tt)*])) $($rest:tt)*) => {{
        let $name = orc_inline_dag!(@const_handle $hc, $reg, $crate::deck![$($tt)*])?;
        orc_inline_dag!(@stmts $ps, $hc, $reg, $($rest)*)
    }};

    // let const with scalar: (let x (const 42.0f64))
    (@stmts $ps:ident, $hc:ident, $reg:ident, (let $name:ident (const $val:expr)) $($rest:tt)*) => {{
        let $name = orc_inline_dag!(@const_handle $hc, $reg, $crate::Deck::from_value($val))?;
        orc_inline_dag!(@stmts $ps, $hc, $reg, $($rest)*)
    }};

    // let single output, more statements follow
    (@stmts $ps:ident, $hc:ident, $reg:ident, (let $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = orc_inline_dag!(@call1 $ps, $hc, $reg, $func, $($arg)*)?;
        orc_inline_dag!(@stmts $ps, $hc, $reg, $($rest)*)
    }};

    // let multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, $reg:ident, (let ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let inputs_ = [$(orc_inline_dag!(@expr $ps, $hc, $reg, $arg)),*];
        let borrowed_ = inputs_.map(|h| h.borrowed());
        let func_ = $ps.get_function(stringify!($func))
            .ok_or($crate::Error::InvalidFunction)?
            .func
            .ok_or($crate::Error::NullPointer)?;
        const N_OUTS_: u64 = orc_inline_dag!(@count $($name)+) as u64;
        let base_handle_ = $hc.fetch_add(N_OUTS_, std::sync::atomic::Ordering::Relaxed);
        let mut outs_: [OrcHandle; N_OUTS_ as usize] = std::array::from_fn(|i| {
            let mut h = OrcHandle::default();
            h.handle = base_handle_ + i as u64;
            h
        });
        unsafe {
            (func_)(0, borrowed_.as_ptr().cast(), borrowed_.len() as u64, outs_.as_mut_ptr(), N_OUTS_);
        }
        let mut idx_ = 0usize;
        $(
            let $name = std::mem::take(&mut outs_[idx_]);
            idx_ += 1;
        )+
        let _ = idx_;
        orc_inline_dag!(@stmts $ps, $hc, $reg, $($rest)*)
    }};

    // Return multiple (or single) named handles as a tuple.
    (@stmts $ps:ident, $hc:ident, $reg:ident, (return $($name:ident)+)) => {
        Ok(($($name),+))
    };

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, $hc:ident, $reg:ident, ($func:ident $($arg:tt)*)) => {
        orc_inline_dag!(@call1 $ps, $hc, $reg, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident, $hc:ident, $reg:ident,) => { Ok::<(), $crate::Error>(()) };

    // --- Expression: either a variable reference, a nested function call, or const ---
    // Const list: (const [1, 2, 3])
    (@expr $ps:ident, $hc:ident, $reg:ident, (const [$($tt:tt)*])) => {
        &orc_inline_dag!(@const_handle $hc, $reg, $crate::deck![$($tt)*])?
    };
    // Const scalar: (const 42.0f64)
    (@expr $ps:ident, $hc:ident, $reg:ident, (const $val:expr)) => {
        &orc_inline_dag!(@const_handle $hc, $reg, $crate::Deck::from_value($val))?
    };
    // Nested call: (func args...)
    (@expr $ps:ident, $hc:ident, $reg:ident, ($func:ident $($arg:tt)*)) => {
        &orc_inline_dag!(@call1 $ps, $hc, $reg, $func, $($arg)*)?
    };
    // Variable reference
    (@expr $ps:ident, $hc:ident, $reg:ident, $var:ident) => {
        &$var
    };

    // --- Const handle: allocate a deck in the registry and return an owning handle ---
    (@const_handle $hc:ident, $reg:ident, $deck:expr) => {{
        let mut h_ = OrcHandle::default();
        h_.handle = $hc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        $reg.alloc_with_value($deck, &mut h_)?;
        Ok::<OrcHandle, $crate::Error>(h_)
    }};

    // --- Single-output function call ---
    (@call1 $ps:ident, $hc:ident, $reg:ident, $func:ident, $($arg:tt)*) => {{
        let inputs_ = [$(orc_inline_dag!(@expr $ps, $hc, $reg, $arg)),*];
        let borrowed_ = inputs_.map(|h| h.borrowed());
        let mut out_: OrcHandle = OrcHandle::default();
        out_.handle = $hc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let func_ = $ps.get_function(stringify!($func))
            .ok_or($crate::Error::InvalidFunction)?
            .func
            .ok_or($crate::Error::NullPointer)?;
        unsafe {
            (func_)(0, borrowed_.as_ptr().cast(), borrowed_.len() as u64, &mut out_, 1);
        }
        Ok::<OrcHandle, $crate::Error>(out_)
    }};


    // --- Counting ---
    (@count $x:ident) => { 1usize };
    (@count $x:ident $($rest:ident)+) => { 1usize + orc_inline_dag!(@count $($rest)+) };

    // --- Error messages for wrong usage ---
    ($ps:expr, { $($body:tt)* }) => {
        compile_error!("orc_inline_dag! requires 4 arguments: orc_inline_dag!(plugin_set, handle_counter, registry, { ... })")
    };
    ($ps:expr) => {
        compile_error!("orc_inline_dag! requires 4 arguments: orc_inline_dag!(plugin_set, handle_counter, registry, { ... })")
    };
}

/// Declarative macro for building a workflow DAG from plugin function calls.
///
/// Takes a PluginSet, an `&AtomicU64` handle counter, a `&DeckRegistry`, a
/// `&mut Workflow`, and a block of statements. Instead of immediately executing
/// functions, this macro adds nodes to the workflow graph and connects them.
///
/// Expressions return `OH` (output handles in the graph) instead of `OrcHandle`.
///
/// ```ignore
/// use std::sync::atomic::AtomicU64;
/// let hc = AtomicU64::new(0);
/// let mut workflow = Workflow::default();
/// orc_dag!(plugin_set, &hc, &registry, &mut workflow, {
///     (let a (const [[1.0, 2.0], [3.0]]))
///     (let b (const [10.0, 20.0, 30.0]))
///     (let sum (add a b))
///     (let (real imag) (complex_get_parts sum))
///     (return real imag)
/// })
/// ```
#[macro_export]
macro_rules! orc_dag {
    // Entry point.
    ($ps:expr, $hc:expr, $reg:expr, $wf:expr, { $($body:tt)* }) => {
        (|| -> Result<_, $crate::DagError> {
            #[allow(unused_variables)]
            let ps_ref_ = &$ps;
            #[allow(unused_variables)]
            let hc_ref_: &std::sync::atomic::AtomicU64 = $hc;
            #[allow(unused_variables)]
            let reg_ref_: &$crate::DeckRegistry = $reg;
            #[allow(unused_variables)]
            let wf_ref_: &mut $crate::Workflow = $wf;
            orc_dag!(@stmts ps_ref_, hc_ref_, reg_ref_, wf_ref_, $($body)*)
        })()
    };

    // --- Statements ---

    // let const with list literal: (let x (const [1, 2, 3]))
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, (let $name:ident (const [$($tt:tt)*])) $($rest:tt)*) => {{
        let $name = orc_dag!(@const_node $hc, $reg, $wf, $crate::deck![$($tt)*])?;
        orc_dag!(@stmts $ps, $hc, $reg, $wf, $($rest)*)
    }};

    // let const with scalar: (let x (const 42.0f64))
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, (let $name:ident (const $val:expr)) $($rest:tt)*) => {{
        let $name = orc_dag!(@const_node $hc, $reg, $wf, $crate::Deck::from_value($val))?;
        orc_dag!(@stmts $ps, $hc, $reg, $wf, $($rest)*)
    }};

    // let single output, more statements follow
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, (let $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = orc_dag!(@call1 $ps, $hc, $reg, $wf, $func, $($arg)*)?;
        orc_dag!(@stmts $ps, $hc, $reg, $wf, $($rest)*)
    }};

    // let multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, (let ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let input_ohs_ = [$(orc_dag!(@expr $ps, $hc, $reg, $wf, $arg)),*];
        let func_info_ = $ps.get_function(stringify!($func))
            .ok_or($crate::DagError::InvalidFunction)?
            .clone();
        const N_INS_: usize = orc_dag!(@count_tt $($arg)*);
        const N_OUTS_: usize = orc_dag!(@count $($name)+);
        let mut ihs_: [$crate::IH; N_INS_] = std::array::from_fn(|_| $crate::IH { idx: 0 });
        let mut ohs_: [$crate::OH; N_OUTS_] = std::array::from_fn(|_| $crate::OH { idx: 0 });
        $wf.add_function(func_info_, &mut ihs_, &mut ohs_)?;
        for (ih_, oh_) in ihs_.iter().zip(input_ohs_.iter()) {
            $wf.connect(*oh_, *ih_)?;
        }
        let mut idx_ = 0usize;
        $(
            let $name = ohs_[idx_];
            idx_ += 1;
        )+
        let _ = idx_;
        orc_dag!(@stmts $ps, $hc, $reg, $wf, $($rest)*)
    }};

    // Return multiple (or single) named handles as a tuple.
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, (return $($name:ident)+)) => {
        Ok(($($name),+))
    };

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $reg, $wf, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident, $hc:ident, $reg:ident, $wf:ident,) => { Ok::<(), $crate::DagError>(()) };

    // --- Expression: either a variable reference, a nested function call, or const ---
    // Const list: (const [1, 2, 3])
    (@expr $ps:ident, $hc:ident, $reg:ident, $wf:ident, (const [$($tt:tt)*])) => {
        orc_dag!(@const_node $hc, $reg, $wf, $crate::deck![$($tt)*])?
    };
    // Const scalar: (const 42.0f64)
    (@expr $ps:ident, $hc:ident, $reg:ident, $wf:ident, (const $val:expr)) => {
        orc_dag!(@const_node $hc, $reg, $wf, $crate::Deck::from_value($val))?
    };
    // Nested call: (func args...)
    (@expr $ps:ident, $hc:ident, $reg:ident, $wf:ident, ($func:ident $($arg:tt)*)) => {
        orc_dag!(@call1 $ps, $hc, $reg, $wf, $func, $($arg)*)?
    };
    // Variable reference
    (@expr $ps:ident, $hc:ident, $reg:ident, $wf:ident, $var:ident) => {
        $var
    };

    // --- Const node: allocate a deck in the registry, create a constant node with 1 output ---
    (@const_node $hc:ident, $reg:ident, $wf:ident, $deck:expr) => {{
        let mut h_ = $crate::OrcHandle::default();
        h_.handle = $hc.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        $reg.alloc_with_value($deck, &mut h_)
            .map_err(|e| $crate::DagError::from(e))?;
        let mut oh_ = $crate::OH { idx: 0 };
        $wf.add_constant(h_, &mut oh_)?;
        Ok::<$crate::OH, $crate::DagError>(oh_)
    }};

    // --- Single-output function call ---
    (@call1 $ps:ident, $hc:ident, $reg:ident, $wf:ident, $func:ident, $($arg:tt)*) => {{
        let input_ohs_ = [$(orc_dag!(@expr $ps, $hc, $reg, $wf, $arg)),*];
        let func_info_ = $ps.get_function(stringify!($func))
            .ok_or($crate::DagError::InvalidFunction)?
            .clone();
        const N_INS_: usize = orc_dag!(@count_tt $($arg)*);
        let mut ihs_: [$crate::IH; N_INS_] = std::array::from_fn(|_| $crate::IH { idx: 0 });
        let mut oh_ = $crate::OH { idx: 0 };
        $wf.add_function(func_info_, &mut ihs_, std::slice::from_mut(&mut oh_))?;
        for (ih_, src_oh_) in ihs_.iter().zip(input_ohs_.iter()) {
            $wf.connect(*src_oh_, *ih_)?;
        }
        Ok::<$crate::OH, $crate::DagError>(oh_)
    }};

    // --- Counting ---
    (@count $x:ident) => { 1usize };
    (@count $x:ident $($rest:ident)+) => { 1usize + orc_dag!(@count $($rest)+) };

    // Count token trees (for counting arguments which may be tt groups or idents)
    (@count_tt $x:tt) => { 1usize };
    (@count_tt $x:tt $($rest:tt)+) => { 1usize + orc_dag!(@count_tt $($rest)+) };
}

#[derive(Debug)]
pub enum DagError {
    BorrowedPropertyAccess,
    MismatchedArrayLengths(usize, usize),
    CycleDetected,
    InvalidFunction,
    SdkError(crate::Error),
}

impl From<crate::Error> for DagError {
    fn from(e: crate::Error) -> Self {
        DagError::SdkError(e)
    }
}

#[cfg(test)]
pub mod test {
    use super::{Graph, IH, LH, NH, OH};

    /// Build a simple chain: A(0 in, 1 out) -> B(1 in, 1 out) -> C(1 in, 0 out).
    pub fn chain_graph() -> (Graph, [NH; 3], [IH; 2], [OH; 2], [LH; 2]) {
        let mut g = Graph::default();
        let mut a_in = [];
        let mut a_out = [OH::default()];
        let na = g.push_node(&mut a_in, &mut a_out).unwrap();
        let mut b_in = [IH::default()];
        let mut b_out = [OH::default()];
        let nb = g.push_node(&mut b_in, &mut b_out).unwrap();
        let mut c_in = [IH::default()];
        let mut c_out = [];
        let nc = g.push_node(&mut c_in, &mut c_out).unwrap();
        let l0 = g.push_link(a_out[0], b_in[0]).unwrap();
        let l1 = g.push_link(b_out[0], c_in[0]).unwrap();
        (
            g,
            [na, nb, nc],
            [b_in[0], c_in[0]],
            [a_out[0], b_out[0]],
            [l0, l1],
        )
    }
}
