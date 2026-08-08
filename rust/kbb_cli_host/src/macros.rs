/// Declarative macro for composing a DAG of plugin function calls.
///
/// Takes a PluginSet, a mutable handle counter, and a block of statements.
/// The counter is used to assign unique handle IDs to all output handles.
/// Supports nesting single-output calls: `(add (mul a b) c)`.
///
/// ```ignore
/// let mut hc = 0u64;
/// kbb_dag!(plugin_set, hc, {
///     (define tmp (mul a b))
///     (define result (add tmp c))
///     (define (mean variance) (compute_stats data))
///     (add (mul a b) c)
/// })
/// ```
#[macro_export]
macro_rules! kbb_dag {
    // Entry point: takes a PluginSet, a handle counter, and a block of statements.
    ($ps:expr, $hc:expr, { $($body:tt)* }) => {{
        #[allow(unused_variables)]
        let ps_ref_ = &$ps;
        let hc_ref_ = &mut $hc;
        kbb_dag!(@stmts ps_ref_, hc_ref_, $($body)*)
    }};

    // --- Statements ---

    // define single output, more statements follow
    (@stmts $ps:ident, $hc:ident, (define $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = kbb_dag!(@call1 $ps, $hc, $func, $($arg)*);
        kbb_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // define multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, (define ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        kbb_dag!(@call_n $ps, $hc, ($($name)+) $func, $($arg)*);
        kbb_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // Trailing expression — bare function call as the block's return value
    (@stmts $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        kbb_dag!(@call1 $ps, $hc, $func, $($arg)*)
    };

    // Empty — end of statements
    (@stmts $ps:ident, $hc:ident,) => { () };

    // --- Expression: either a variable reference or a nested function call ---
    // Nested call: (func args...)
    (@expr $ps:ident, $hc:ident, ($func:ident $($arg:tt)*)) => {
        kbb_dag!(@call1 $ps, $hc, $func, $($arg)*)
    };
    // Variable reference
    (@expr $ps:ident, $hc:ident, $var:ident) => {
        unsafe { $var.non_owning_clone() }
    };

    // --- Single-output function call ---
    (@call1 $ps:ident, $hc:ident, $func:ident, $($arg:tt)*) => {{
        let inputs_: Vec<OrcHandle> = vec![$(kbb_dag!(@expr $ps, $hc, $arg)),*];
        let mut out_: OrcHandle = OrcHandle::default();
        out_.handle = *$hc;
        *$hc += 1;
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        unsafe {
            (func_.func)(0, inputs_.as_ptr(), inputs_.len() as u64, &mut out_, 1);
        }
        out_
    }};

    // --- Multi-output function call ---
    (@call_n $ps:ident, $hc:ident, ($($name:ident)+) $func:ident, $($arg:tt)*) => {
        let inputs_: Vec<OrcHandle> = vec![$(kbb_dag!(@expr $ps, $hc, $arg)),*];
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        let n_outs_: u64 = kbb_dag!(@count $($name)+) as u64;
        let mut outs_: Vec<OrcHandle> = (0..n_outs_).map(|i| {
            let mut h = OrcHandle::default();
            h.handle = *$hc + i as u64;
            h
        }).collect();
        *$hc += n_outs_;
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

    // --- Error messages for wrong usage ---
    ($ps:expr, { $($body:tt)* }) => {
        compile_error!("kbb_dag! requires 3 arguments: kbb_dag!(plugin_set, handle_counter, { ... })")
    };
    ($ps:expr) => {
        compile_error!("kbb_dag! requires 3 arguments: kbb_dag!(plugin_set, handle_counter, { ... })")
    };
}
