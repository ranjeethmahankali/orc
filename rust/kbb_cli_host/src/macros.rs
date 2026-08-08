/// Declarative macro for composing a DAG of plugin function calls.
///
/// Each statement is a `define` that calls a plugin function and binds the output(s).
/// Single identifier = one output. Parenthesized list = multiple outputs.
///
/// ```ignore
/// kbb_dag!(plugin_set, {
///     (define tmp (mul a b))
///     (define result (add tmp c))
///     (define (mean variance) (compute_stats data))
/// })
/// ```
///
/// All inputs and outputs are `OrcHandle` variables. Input handles are passed as
/// non-owning clones (no `free_fn`) so they can be safely referenced by multiple calls
/// without double-free. External variables declared outside the macro can be used as inputs.
#[macro_export]
macro_rules! kbb_dag {
    // Entry point: takes a PluginSet reference and a block of define statements.
    ($ps:expr, { $($body:tt)* }) => {{
        #[allow(unused_variables)]
        let ps_ref_ = &$ps;
        kbb_dag!(@stmts ps_ref_, $($body)*)
    }};

    // --- Statements ---

    // define single output, more statements follow
    (@stmts $ps:ident, (define $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {
        let $name = kbb_dag!(@call1 $ps, $func, $($arg)*);
        kbb_dag!(@stmts $ps, $($rest)*)
    };

    // define multiple outputs, more statements follow
    (@stmts $ps:ident, (define ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {
        kbb_dag!(@call_n $ps, ($($name)+) $func, $($arg)*);
        kbb_dag!(@stmts $ps, $($rest)*)
    };

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, ($func:ident $($arg:tt)*)) => {
        kbb_dag!(@call1 $ps, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident,) => { () };

    // --- Single-output function call ---
    (@call1 $ps:ident, $func:ident, $($arg:ident)*) => {{
        let inputs_: &[OrcHandle] = &[$(unsafe { $arg.non_owning_clone() }),*];
        let mut out_: OrcHandle = OrcHandle::default();
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        unsafe {
            (func_.func)(0, inputs_.as_ptr(), inputs_.len() as u64, &mut out_, 1);
        }
        out_
    }};

    // --- Multi-output function call ---
    // We count the output names to determine n_outputs, create an array, then destructure.
    (@call_n $ps:ident, ($($name:ident)+) $func:ident, $($arg:ident)*) => {
        let inputs_: &[OrcHandle] = &[$(unsafe { $arg.non_owning_clone() }),*];
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        let n_outs_: u64 = kbb_dag!(@count $($name)+) as u64;
        let mut outs_: Vec<OrcHandle> = (0..n_outs_).map(|_| OrcHandle::default()).collect();
        unsafe {
            (func_.func)(0, inputs_.as_ptr(), inputs_.len() as u64, outs_.as_mut_ptr(), n_outs_);
        }
        let mut idx_ = 0usize;
        $(
            let $name = std::mem::replace(&mut outs_[idx_], OrcHandle::default());
            idx_ += 1;
        )+
        let _ = idx_;
    };

    // --- Counting ---
    (@count $x:ident) => { 1usize };
    (@count $x:ident $($rest:ident)+) => { 1usize + kbb_dag!(@count $($rest)+) };
}
