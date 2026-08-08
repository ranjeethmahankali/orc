/// Declarative macro for composing a DAG of plugin function calls.
///
/// Takes a PluginSet, an `&AtomicU64` handle counter, and a block of statements.
/// The counter is used to assign unique handle IDs to all output handles.
/// Supports nesting single-output calls: `(add (mul a b) c)`.
///
/// ```ignore
/// use std::sync::atomic::AtomicU64;
/// let hc = AtomicU64::new(0);
/// kbb_dag!(plugin_set, &hc, {
///     (let tmp (mul a b))
///     (let result (add tmp c))
///     (let (mean variance) (compute_stats data))
///     (add (mul a b) c)
/// })
/// ```
#[macro_export]
macro_rules! kbb_dag {
    // Entry point: takes a PluginSet, an &AtomicU64 handle counter, and a block of statements.
    ($ps:expr, $hc:expr, { $($body:tt)* }) => {{
        #[allow(unused_variables)]
        let ps_ref_ = &$ps;
        #[allow(unused_variables)]
        let hc_ref_: &std::sync::atomic::AtomicU64 = $hc;
        kbb_dag!(@stmts ps_ref_, hc_ref_, $($body)*)
    }};

    // --- Statements ---

    // let single output, more statements follow
    (@stmts $ps:ident, $hc:ident, (let $name:ident ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
        let $name = kbb_dag!(@call1 $ps, $hc, $func, $($arg)*);
        kbb_dag!(@stmts $ps, $hc, $($rest)*)
    }};

    // let multiple outputs, more statements follow
    (@stmts $ps:ident, $hc:ident, (let ($($name:ident)+) ($func:ident $($arg:tt)*)) $($rest:tt)*) => {{
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
        let inputs_: Vec<OrcHandle> = vec![$(kbb_dag!(@expr $ps, $hc, $arg)),*];
        let func_ = $ps.get_function(stringify!($func))
            .expect(concat!("function '", stringify!($func), "' not found"));
        let n_outs_: u64 = kbb_dag!(@count $($name)+) as u64;
        let base_handle_ = $hc.fetch_add(n_outs_, std::sync::atomic::Ordering::Relaxed);
        let mut outs_: Vec<OrcHandle> = (0..n_outs_).map(|i| {
            let mut h = OrcHandle::default();
            h.handle = base_handle_ + i;
            h
        }).collect();
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

#[cfg(test)]
mod tests {
    use crate::test::{HANDLE_COUNTER, PLUGIN_SET};
    use orc_sdk::{Deck, DeckView, OrcHandle, deck, update_handle_from_deck};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn make_handle(hc: &AtomicU64, deck: &Deck<f64>) -> OrcHandle {
        let mut h = OrcHandle {
            handle: hc.fetch_add(1, Ordering::Relaxed),
            ..Default::default()
        };
        unsafe { update_handle_from_deck(deck, &mut h) };
        h
    }

    // 1. Single function call as trailing expression.
    #[test]
    fn t_single_call() {
        let a_deck: Deck<f64> = deck![1.0, 2.0, 3.0];
        let b_deck: Deck<f64> = deck![10.0, 20.0, 30.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (add a b)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
    }

    // 2. let binding followed by trailing expression.
    #[test]
    fn t_let_then_trailing() {
        let a_deck: Deck<f64> = deck![1.0, 2.0, 3.0];
        let b_deck: Deck<f64> = deck![10.0, 20.0, 30.0];
        let c_deck: Deck<f64> = deck![100.0, 200.0, 300.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        let c = make_handle(&HANDLE_COUNTER, &c_deck);
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (let ab (add a b))
            (add ab c)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[111.0, 222.0, 333.0]);
    }

    // 3. Multiple let bindings.
    #[test]
    fn t_multiple_lets() {
        let a_deck: Deck<f64> = deck![2.0, 3.0];
        let b_deck: Deck<f64> = deck![5.0, 10.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        // (a + b) * (a + b) = (7, 13) * (7, 13) = (49, 169)
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (let sum (add a b))
            (mul sum sum)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[49.0, 169.0]);
    }

    // 4. Nested single-output call.
    #[test]
    fn t_nested_call() {
        let a_deck: Deck<f64> = deck![2.0, 3.0];
        let b_deck: Deck<f64> = deck![5.0, 10.0];
        let c_deck: Deck<f64> = deck![1.0, 1.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        let c = make_handle(&HANDLE_COUNTER, &c_deck);
        // (a * b) + c = (10, 30) + (1, 1) = (11, 31)
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (add (mul a b) c)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[11.0, 31.0]);
    }

    // 5. Deeply nested calls.
    #[test]
    fn t_deep_nesting() {
        let a_deck: Deck<f64> = deck![1.0, 2.0];
        let b_deck: Deck<f64> = deck![3.0, 4.0];
        let c_deck: Deck<f64> = deck![10.0, 10.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        let c = make_handle(&HANDLE_COUNTER, &c_deck);
        // (a + b) * c = (4, 6) * (10, 10) = (40, 60)
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (mul (add a b) c)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[40.0, 60.0]);
    }

    // 6. let binding with nested call in the body.
    #[test]
    fn t_let_with_nested() {
        let a_deck: Deck<f64> = deck![2.0];
        let b_deck: Deck<f64> = deck![3.0];
        let c_deck: Deck<f64> = deck![10.0];
        let a = make_handle(&HANDLE_COUNTER, &a_deck);
        let b = make_handle(&HANDLE_COUNTER, &b_deck);
        let c = make_handle(&HANDLE_COUNTER, &c_deck);
        // let prod = a * b = 6; prod + c = 16; result - prod = 16 - 6 = 10
        let out = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (let prod (mul a b))
            (sub (add prod c) prod)
        });
        let view = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(view.items(), &[10.0]);
    }

    // 7. Handle counter increments correctly.
    #[test]
    fn t_handle_counter() {
        let hc = AtomicU64::new(0);
        let a_deck: Deck<f64> = deck![1.0];
        let b_deck: Deck<f64> = deck![2.0];
        let a = make_handle(&hc, &a_deck);
        let b = make_handle(&hc, &b_deck);
        assert_eq!(hc.load(Ordering::Relaxed), 2); // a=0, b=1
        let _out = kbb_dag!(*PLUGIN_SET, &hc, {
            (add a b)
        });
        assert_eq!(hc.load(Ordering::Relaxed), 3); // one output handle allocated
    }

    // 8. Handle counter increments correctly with nested calls.
    #[test]
    fn t_handle_counter_nested() {
        let hc = AtomicU64::new(0);
        let a_deck: Deck<f64> = deck![1.0];
        let b_deck: Deck<f64> = deck![2.0];
        let c_deck: Deck<f64> = deck![3.0];
        let a = make_handle(&hc, &a_deck);
        let b = make_handle(&hc, &b_deck);
        let c = make_handle(&hc, &c_deck);
        assert_eq!(hc.load(Ordering::Relaxed), 3);
        // Nested: (mul a b) allocates 1 handle, (add _ c) allocates 1 handle
        let _out = kbb_dag!(*PLUGIN_SET, &hc, {
            (add (mul a b) c)
        });
        assert_eq!(hc.load(Ordering::Relaxed), 5);
    }

    // 9. External variables are usable as inputs.
    #[test]
    fn t_external_variables() {
        let x_deck: Deck<f64> = deck![100.0];
        let y_deck: Deck<f64> = deck![1.0];
        let x = make_handle(&HANDLE_COUNTER, &x_deck);
        let y = make_handle(&HANDLE_COUNTER, &y_deck);
        let result = kbb_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
            (sub x y)
        });
        let view = DeckView::<f64>::from_handle(&result).unwrap();
        assert_eq!(view.items(), &[99.0]);
    }

    // 10. Empty block returns unit.
    #[test]
    fn t_empty_block() {
        let hc = AtomicU64::new(0);
        assert_eq!(kbb_dag!(*PLUGIN_SET, &hc, {}), ());
    }
}
