use crate::{HANDLE_COUNTER, PLUGIN_SET, REGISTRY, SERIAL_CONTEXT_ARENA, host_clone_orc_handle};
use orc_sdk::{
    DagError, Deck, DeckView, IH, OH, OrcHandle, OrcTypeId, Plugin, TypeOwner, Workflow, deck,
    orc_dag, orc_inline_dag, update_handle_from_deck,
};
use std::sync::atomic::Ordering;

fn next_id() -> u64 {
    HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn t_add_fn() {
    let add_fn = PLUGIN_SET
        .get_function("add")
        .expect("add function not found");
    let a: Deck<f64> = deck![[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]];
    let b: Deck<f64> = deck![10.0, 20.0, 30.0];
    let mut a_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let mut b_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let mut out_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    unsafe {
        update_handle_from_deck(&a, &mut a_handle);
        update_handle_from_deck(&b, &mut b_handle);
    }
    let inputs: &[OrcHandle] = &[a_handle, b_handle];
    unsafe {
        (add_fn.func.expect("Invalid function"))(
            0,
            inputs.as_ptr(),
            inputs.len() as u64,
            &mut out_handle,
            1,
        );
    }
    let view = DeckView::<f64>::from_handle(&out_handle).unwrap();
    const EXPECTED: &[&[f64]] = &[&[11.0, 22.0, 33.0], &[12.0, 24.0, 36.0, 38.0]];
    assert_eq!(view.depth(), 2);
    for (child, expected) in view.child().advance_iter().zip(EXPECTED.iter()) {
        assert_eq!(child.depth(), 1);
        assert_eq!(child.as_slice(), *expected);
    }
}

#[test]
fn t_list_length_fn() {
    let list_length_fn = PLUGIN_SET
        .get_function("list_length")
        .expect("list_length function not found");
    let a: Deck<f64> = deck![[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]];
    let mut a_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let mut out_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    unsafe {
        update_handle_from_deck(&a, &mut a_handle);
    }
    let inputs: &[OrcHandle] = &[a_handle];
    unsafe {
        (list_length_fn.func.expect("Invalid function"))(
            0,
            inputs.as_ptr(),
            inputs.len() as u64,
            &mut out_handle,
            1,
        );
    }
    let view = DeckView::<u64>::from_handle(&out_handle).unwrap();
    assert_eq!(view.items(), &[3, 4]);
}

#[test]
fn t_flatten_deck_fn() {
    let flatten_fn = PLUGIN_SET
        .get_function("flatten_deck")
        .expect("flatten_deck function not found");
    let a: Deck<f64> = deck![[1.0, 2.0, 3.0], [4.0, 5.0]];
    let mut a_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let mut out_handle = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    unsafe {
        update_handle_from_deck(&a, &mut a_handle);
        (flatten_fn.func.expect("Invalid function"))(0, &a_handle, 1, &mut out_handle, 1);
    }
    let view = DeckView::<f64>::from_handle(&out_handle).unwrap();
    assert_eq!(view.items(), &[1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn t_flatten_complex() {
    // [[1+0i, 0+1i], [2+0i]] * [[5+0i, 0+1i], [3+0i]] = [[5+0i, -1+0i], [6+0i]]
    // flatten_deck (via proxy) → [5+0i, -1+0i, 6+0i]
    // complex_get_parts → real=[5,-1,6], imag=[0,0,0]
    let create_complex = PLUGIN_SET
        .get_function("create_complex")
        .expect("create_complex not found");
    let mul_complex = PLUGIN_SET
        .get_function("mul_complex")
        .expect("mul_complex not found");
    let flatten_fn = PLUGIN_SET
        .get_function("flatten_deck")
        .expect("flatten_deck not found");
    let get_parts = PLUGIN_SET
        .get_function("complex_get_parts")
        .expect("complex_get_parts not found");
    let lhs_real: Deck<f64> = deck![[1.0, 0.0], [2.0]];
    let lhs_imag: Deck<f64> = deck![[0.0, 1.0], [0.0]];
    let rhs_real: Deck<f64> = deck![[5.0, 0.0], [3.0]];
    let rhs_imag: Deck<f64> = deck![[0.0, 1.0], [0.0]];

    let (mut lhs_real_h, mut lhs_imag_h, mut rhs_real_h, mut rhs_imag_h) = (
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
    );
    unsafe {
        update_handle_from_deck(&lhs_real, &mut lhs_real_h);
        update_handle_from_deck(&lhs_imag, &mut lhs_imag_h);
        update_handle_from_deck(&rhs_real, &mut rhs_real_h);
        update_handle_from_deck(&rhs_imag, &mut rhs_imag_h);
    }

    let mut lhs_complex = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [lhs_real_h, lhs_imag_h];
    unsafe {
        (create_complex.func.expect("Invalid function"))(0, inputs.as_ptr(), 2, &mut lhs_complex, 1)
    };

    let mut rhs_complex = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [rhs_real_h, rhs_imag_h];
    unsafe {
        (create_complex.func.expect("Invalid function"))(0, inputs.as_ptr(), 2, &mut rhs_complex, 1)
    };

    let mut mul_out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [lhs_complex, rhs_complex];
    unsafe {
        (mul_complex.func.expect("Invalid function"))(0, inputs.as_ptr(), 2, &mut mul_out, 1)
    };
    assert!(mul_out.n_marks > 0); // nested: [[5+0i, -1+0i], [6+0i]]

    let mut flat = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    unsafe { (flatten_fn.func.expect("Invalid function"))(0, &mul_out, 1, &mut flat, 1) };

    assert_eq!(flat.n_items, 3);

    let mut parts = [
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        },
    ];
    unsafe { (get_parts.func.expect("Invalid function"))(0, &flat, 1, parts.as_mut_ptr(), 2) };

    let real = DeckView::<f64>::from_handle(&parts[0]).unwrap();
    let imag = DeckView::<f64>::from_handle(&parts[1]).unwrap();
    assert_eq!(real.items(), &[5.0, -1.0, 6.0]);
    assert_eq!(imag.items(), &[0.0, 0.0, 0.0]);
}

// ==================================================
// orc_inline_dag macro tests.
// ==================================================

// 1. Single function call as trailing expression.
#[test]
fn t_single_call() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (add (const [1.0f64, 2.0, 3.0]) (const [10.0f64, 20.0, 30.0]))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
}

// 2. let binding followed by trailing expression.
#[test]
fn t_let_then_trailing() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (let c (const [100.0f64, 200.0, 300.0]))
        (let ab (add a b))
        (add ab c)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[111.0, 222.0, 333.0]);
}

// 3. Multiple let bindings.
#[test]
fn t_multiple_lets() {
    // (a + b) * (a + b) = (7, 13) * (7, 13) = (49, 169)
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [2.0f64, 3.0]))
        (let b (const [5.0f64, 10.0]))
        (let sum (add a b))
        (mul sum sum)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[49.0, 169.0]);
}

// 4. Nested single-output call.
#[test]
fn t_nested_call() {
    // (a * b) + c = (10, 30) + (1, 1) = (11, 31)
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (add (mul (const [2.0f64, 3.0]) (const [5.0f64, 10.0])) (const [1.0f64, 1.0]))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 31.0]);
}

// 5. Deeply nested calls.
#[test]
fn t_deep_nesting() {
    // (a + b) * c = (4, 6) * (10, 10) = (40, 60)
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (mul (add (const [1.0f64, 2.0]) (const [3.0f64, 4.0])) (const [10.0f64, 10.0]))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[40.0, 60.0]);
}

// 6. let binding with nested call in the body.
#[test]
fn t_let_with_nested() {
    // let prod = a * b = 6; prod + c = 16; result - prod = 16 - 6 = 10
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let prod (mul (const 2.0f64) (const 3.0f64)))
        (sub (add prod (const 10.0f64)) prod)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[10.0]);
}

// 9. Const scalars used as inputs.
#[test]
fn t_const_inputs() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (sub (const 100.0f64) (const 1.0f64))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[99.0]);
}

// 10. Empty block returns unit.
#[test]
fn t_empty_block() {
    assert_eq!(
        orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {}).unwrap(),
        ()
    );
}

// 11. Const scalar via let binding.
#[test]
fn t_const_scalar() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const 3.0f64))
        (let b (const 7.0f64))
        (add a b)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[10.0]);
    assert_eq!(view.depth(), 0);
}

// 12. Const list via let binding.
#[test]
fn t_const_list() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (add a b)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
    assert_eq!(view.depth(), 1);
}

// 13. Const nested list.
#[test]
fn t_const_nested_list() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [[1.0f64, 2.0], [3.0]]))
        (flatten_deck a)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[1.0, 2.0, 3.0]);
    assert_eq!(view.depth(), 1);
}

// 14. Const used inline as expression argument.
#[test]
fn t_const_inline_expr() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (add (const [2.0f64, 4.0]) (const [10.0f64, 20.0]))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[12.0, 24.0]);
    assert_eq!(view.depth(), 1);
}

// 15. Const mixed with let-bound const.
#[test]
fn t_const_mixed() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [5.0f64, 10.0]))
        (add a (const [1.0f64, 2.0]))
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[6.0, 12.0]);
    assert_eq!(view.depth(), 1);
}

// 16. Const depth 2: nested list added element-wise.
#[test]
fn t_const_depth2() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [[1.0f64, 2.0], [3.0]]))
        (let b (const [[10.0f64, 20.0], [30.0]]))
        (add a b)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
    assert_eq!(view.depth(), 2);
}

// 17. Const depth 3.
#[test]
fn t_const_depth3() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [[[1.0f64, 2.0], [3.0]], [[4.0]]]))
        (let b (const [[[10.0f64, 20.0], [30.0]], [[40.0]]]))
        (add a b)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0, 44.0]);
    assert_eq!(view.depth(), 3);
}

// 18. Return multiple outputs.
#[test]
fn t_return_multiple() {
    let (sum, product) = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [1.0f64, 2.0]))
        (let b (const [10.0f64, 20.0]))
        (let s (add a b))
        (let p (mul a b))
        (return s p)
    })
    .unwrap();
    let sum_view = DeckView::<f64>::from_handle(&sum).unwrap();
    assert_eq!(sum_view.items(), &[11.0, 22.0]);
    assert_eq!(sum_view.depth(), 1);
    let prod_view = DeckView::<f64>::from_handle(&product).unwrap();
    assert_eq!(prod_view.items(), &[10.0, 40.0]);
    assert_eq!(prod_view.depth(), 1);
}

// 19. Return single output (same as trailing expression, but explicit).
#[test]
fn t_return_single() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (let s (add a b))
        (return s)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
    assert_eq!(view.depth(), 1);
}

// 20. Const scalar reused in multiple expressions.
#[test]
fn t_const_reused() {
    let out = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let x (const [3.0f64, 4.0]))
        (mul x x)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[9.0, 16.0]);
    assert_eq!(view.depth(), 1);
}

// ==================================================
// orc_dag macro tests (build graph, then run).
// ==================================================

fn run_dag_single(wf: &Workflow) -> OrcHandle {
    let mut out = [OrcHandle::default()];
    wf.run(&[], &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let [o] = out;
    o
}

fn assert_dag_output_f64(data: OrcHandle, expected: &[f64], expected_depth: u8) {
    let view = DeckView::<f64>::from_handle(&data).unwrap();
    assert_eq!(view.items(), expected);
    assert_eq!(view.depth(), expected_depth);
}

// 1. Single function call as trailing expression.
#[test]
fn t_dag_single_call() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add (const [1.0f64, 2.0, 3.0]) (const [10.0f64, 20.0, 30.0]))
    })
    .unwrap();
    // Trailing expression: unnamed output.
    assert_eq!(wf.workflow_outputs().len(), 1);
    assert_eq!(wf.workflow_outputs()[0].1, "");
    assert!(wf.workflow_inputs().unwrap().is_empty());
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 22.0, 33.0], 1);
}

// 2. let binding followed by trailing expression.
#[test]
fn t_dag_let_then_trailing() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (let c (const [100.0f64, 200.0, 300.0]))
        (let ab (add a b))
        (add ab c)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[111.0, 222.0, 333.0], 1);
}

// 3. Multiple let bindings — shared subexpression.
#[test]
fn t_dag_multiple_lets() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [2.0f64, 3.0]))
        (let b (const [5.0f64, 10.0]))
        (let sum (add a b))
        (mul sum sum)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[49.0, 169.0], 1);
}

// 4. Nested single-output call.
#[test]
fn t_dag_nested_call() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add (mul (const [2.0f64, 3.0]) (const [5.0f64, 10.0])) (const [1.0f64, 1.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 31.0], 1);
}

// 5. Deeply nested calls.
#[test]
fn t_dag_deep_nesting() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (mul (add (const [1.0f64, 2.0]) (const [3.0f64, 4.0])) (const [10.0f64, 10.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[40.0, 60.0], 1);
}

// 6. let binding with nested call in the body.
#[test]
fn t_dag_let_with_nested() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let prod (mul (const 2.0f64) (const 3.0f64)))
        (sub (add prod (const 10.0f64)) prod)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[10.0], 0);
}

// 9. Const scalars used as inputs.
#[test]
fn t_dag_const_inputs() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (sub (const 100.0f64) (const 1.0f64))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[99.0], 0);
}

// 11. Const scalar via let binding.
#[test]
fn t_dag_const_scalar() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const 3.0f64))
        (let b (const 7.0f64))
        (add a b)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[10.0], 0);
}

// 12. Const list via let binding.
#[test]
fn t_dag_const_list() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (add a b)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 22.0, 33.0], 1);
}

// 14. Const used inline as expression argument.
#[test]
fn t_dag_const_inline_expr() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add (const [2.0f64, 4.0]) (const [10.0f64, 20.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[12.0, 24.0], 1);
}

// 15. Const mixed with let-bound const.
#[test]
fn t_dag_const_mixed() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [5.0f64, 10.0]))
        (add a (const [1.0f64, 2.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[6.0, 12.0], 1);
}

// 16. Const depth 2: nested list added element-wise.
#[test]
fn t_dag_const_depth2() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [[1.0f64, 2.0], [3.0]]))
        (let b (const [[10.0f64, 20.0], [30.0]]))
        (add a b)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 22.0, 33.0], 2);
}

// 17. Const depth 3.
#[test]
fn t_dag_const_depth3() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [[[1.0f64, 2.0], [3.0]], [[4.0]]]))
        (let b (const [[[10.0f64, 20.0], [30.0]], [[40.0]]]))
        (add a b)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 22.0, 33.0, 44.0], 3);
}

// 18. Return multiple outputs.
#[test]
fn t_dag_return_multiple() {
    let mut wf = Workflow::default();
    let (_s_oh, _p_oh) = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [1.0f64, 2.0]))
        (let b (const [10.0f64, 20.0]))
        (let s (add a b))
        (let p (mul a b))
        (return s p)
    })
    .unwrap();
    // Return names match the let-bound identifiers.
    assert_eq!(wf.workflow_outputs().len(), 2);
    assert_eq!(wf.workflow_outputs()[0].1, "s");
    assert_eq!(wf.workflow_outputs()[1].1, "p");
    assert!(wf.workflow_inputs().unwrap().is_empty());
    let mut out = [OrcHandle::default(), OrcHandle::default()];
    wf.run(&[], &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let [s_out, p_out] = out;
    assert_dag_output_f64(s_out, &[11.0, 22.0], 1);
    assert_dag_output_f64(p_out, &[10.0, 40.0], 1);
}

// 19. Return single output (explicit).
#[test]
fn t_dag_return_single() {
    let mut wf = Workflow::default();
    let _s_oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [1.0f64, 2.0, 3.0]))
        (let b (const [10.0f64, 20.0, 30.0]))
        (let s (add a b))
        (return s)
    })
    .unwrap();
    assert_eq!(wf.workflow_outputs().len(), 1);
    assert_eq!(wf.workflow_outputs()[0].1, "s");
    assert_dag_output_f64(run_dag_single(&wf), &[11.0, 22.0, 33.0], 1);
}

// 20. Const reused in multiple expressions.
#[test]
fn t_dag_const_reused() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let x (const [3.0f64, 4.0]))
        (mul x x)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[9.0, 16.0], 1);
}

// 13. Const nested list through flatten.
#[test]
fn t_dag_const_nested_list() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [[1.0f64, 2.0], [3.0]]))
        (flatten_deck a)
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[1.0, 2.0, 3.0], 1);
}

// ==================================================
// Cycle detection tests.
// ==================================================

// A simple cycle: A -> B -> A. run() should return CycleDetected.
#[test]
fn t_dag_cycle_simple() {
    let mut wf = Workflow::default();
    let mut a_in = [IH::default()];
    let mut a_out = [OH::default()];
    let func = PLUGIN_SET.get_function("add").unwrap().clone();
    wf.add_function(func.clone(), &mut a_in, &mut a_out)
        .unwrap();
    let mut b_in = [IH::default()];
    let mut b_out = [OH::default()];
    wf.add_function(func, &mut b_in, &mut b_out).unwrap();
    // A's output -> B's input
    wf.connect(a_out[0], b_in[0]).unwrap();
    // B's output -> A's input (cycle!)
    wf.connect(b_out[0], a_in[0]).unwrap();
    wf.set_outputs(&[(b_out[0], String::new())]).unwrap();
    let mut out = [OrcHandle::default()];
    let result = wf.run(&[], &mut out, &host_clone_orc_handle, &HANDLE_COUNTER);
    assert!(matches!(result, Err(DagError::CycleDetected)));
}

// Self-cycle: A -> A.
#[test]
fn t_dag_cycle_self() {
    let mut wf = Workflow::default();
    let mut a_in = [IH::default()];
    let mut a_out = [OH::default()];
    let func = PLUGIN_SET.get_function("add").unwrap().clone();
    wf.add_function(func, &mut a_in, &mut a_out).unwrap();
    // A's output -> A's input (self-cycle!)
    wf.connect(a_out[0], a_in[0]).unwrap();
    wf.set_outputs(&[(a_out[0], String::new())]).unwrap();
    let mut out = [OrcHandle::default()];
    let result = wf.run(&[], &mut out, &host_clone_orc_handle, &HANDLE_COUNTER);
    assert!(matches!(result, Err(DagError::CycleDetected)));
}

// Diamond is NOT a cycle — should succeed.
#[test]
fn t_dag_diamond_no_cycle() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let x (const [2.0f64, 3.0]))
        (let a (add x x))
        (let b (mul x x))
        (add a b)
    })
    .unwrap();
    // x = [2, 3], a = x+x = [4, 6], b = x*x = [4, 9], result = a+b = [8, 15]
    assert_dag_output_f64(run_dag_single(&wf), &[8.0, 15.0], 1);
}

// ==================================================
// Workflow input tests.
// ==================================================

// Workflow input used as input to a function.
#[test]
fn t_dag_input_basic() {
    let mut wf = Workflow::default();
    // 'lhs is a workflow input; rhs is a constant.
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add 'lhs (const [10.0f64, 20.0, 30.0]))
    })
    .unwrap();
    // One input named "lhs" at index 0, one unnamed output.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 1);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "lhs");
    assert_eq!(wf.workflow_outputs().len(), 1);
    assert_eq!(wf.workflow_outputs()[0].1, "");
    // Run with lhs = [1, 2, 3].
    let x_handle = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let x (const [1.0f64, 2.0, 3.0]))
        (return x)
    })
    .unwrap();
    let inputs = [x_handle.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
}

// Re-run with different input values. Both inputs share the same quoted symbol.
#[test]
fn t_dag_input_rerun() {
    let mut wf = Workflow::default();
    // 'x used twice — both inputs of mul receive the same workflow input (x * x).
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (mul 'x 'x)
    })
    .unwrap();
    // Two IHs both named "x" at index 0 (deduped). One unnamed output.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 2);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "x");
    assert_eq!(wf_ins[1].1, 0);
    assert_eq!(wf_ins[1].2, "x");
    assert_eq!(wf.workflow_outputs().len(), 1);
    // First run: x = [3, 4]
    let h1 = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let v (const [3.0f64, 4.0]))
        (return v)
    })
    .unwrap();
    let inputs = [h1.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[9.0, 16.0]);
    // Second run: x = [5, 6]
    let h2 = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let v (const [5.0f64, 6.0]))
        (return v)
    })
    .unwrap();
    let inputs = [h2.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[25.0, 36.0]);
}

// ==================================================
// Garbage collection tests.
// ==================================================

// Workflow outputs survive GC and the workflow still runs correctly.
#[test]
fn t_dag_gc_outputs_preserved() {
    let mut wf = Workflow::default();
    let (_s_oh, _p_oh) = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (let a (const [1.0f64, 2.0]))
        (let b (const [10.0f64, 20.0]))
        (let s (add a b))
        (let p (mul a b))
        (return s p)
    })
    .unwrap();
    // Verify outputs before GC.
    assert_eq!(wf.workflow_outputs().len(), 2);
    assert_eq!(wf.workflow_outputs()[0].1, "s");
    assert_eq!(wf.workflow_outputs()[1].1, "p");
    // Run GC.
    wf.garbage_collection().unwrap();
    // Outputs preserved: same count, same names, same order.
    assert_eq!(wf.workflow_outputs().len(), 2);
    assert_eq!(wf.workflow_outputs()[0].1, "s");
    assert_eq!(wf.workflow_outputs()[1].1, "p");
    // Workflow still produces correct results.
    let mut out = [OrcHandle::default(), OrcHandle::default()];
    wf.run(&[], &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let [s_out, p_out] = out;
    assert_dag_output_f64(s_out, &[11.0, 22.0], 1);
    assert_dag_output_f64(p_out, &[10.0, 40.0], 1);
}

// Workflow inputs survive GC and the workflow still runs correctly.
#[test]
fn t_dag_gc_inputs_preserved() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add 'lhs (const [10.0f64, 20.0, 30.0]))
    })
    .unwrap();
    // Verify inputs before GC.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 1);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "lhs");
    // Run GC.
    wf.garbage_collection().unwrap();
    // Inputs preserved.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 1);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "lhs");
    // Workflow still runs correctly.
    let x_handle = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let x (const [1.0f64, 2.0, 3.0]))
        (return x)
    })
    .unwrap();
    let inputs = [x_handle.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
}

// Deduped inputs ('x 'x) survive GC with correct shared index.
#[test]
fn t_dag_gc_dedup_inputs_preserved() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (mul 'x 'x)
    })
    .unwrap();
    // Verify before GC.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 2);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "x");
    assert_eq!(wf_ins[1].1, 0);
    assert_eq!(wf_ins[1].2, "x");
    // Run GC.
    wf.garbage_collection().unwrap();
    // Inputs preserved with shared index.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 2);
    assert_eq!(wf_ins[0].1, 0);
    assert_eq!(wf_ins[0].2, "x");
    assert_eq!(wf_ins[1].1, 0);
    assert_eq!(wf_ins[1].2, "x");
    // Still runs correctly: x = [3, 4], x*x = [9, 16].
    let h = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let v (const [3.0f64, 4.0]))
        (return v)
    })
    .unwrap();
    let inputs = [h.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[9.0, 16.0]);
}

// ==================================================
// Nested workflow tests.
// ==================================================

// Define a nested function and call it.
#[test]
fn t_dag_nested_fn_basic() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (fn double
            (add 'x 'x))
        (double (const [3.0f64, 5.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&wf), &[6.0, 10.0], 1);
}

// Nested function with multiple statements in its body.
#[test]
fn t_dag_nested_fn_multi_stmt() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (fn sum_of_squares
            (let a (mul 'x 'x))
            (let b (mul 'y 'y))
            (add a b))
        (sum_of_squares (const [3.0f64]) (const [4.0f64]))
    })
    .unwrap();
    // 3^2 + 4^2 = 9 + 16 = 25
    assert_dag_output_f64(run_dag_single(&wf), &[25.0], 1);
}

// Nested function called multiple times with different arguments.
#[test]
fn t_dag_nested_fn_reuse() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (fn double
            (add 'x 'x))
        (let a (double (const [1.0f64, 2.0])))
        (let b (double (const [10.0f64, 20.0])))
        (add a b)
    })
    .unwrap();
    // double([1,2]) = [2,4], double([10,20]) = [20,40], sum = [22,44]
    assert_dag_output_f64(run_dag_single(&wf), &[22.0, 44.0], 1);
}

// Nested function with multiple outputs.
#[test]
fn t_dag_nested_fn_multi_output() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (fn sum_and_product
            (let s (add 'a 'b))
            (let p (mul 'a 'b))
            (return s p))
        // Now call the nested workflow.
        (let (s p) (sum_and_product (const [3.0f64]) (const [4.0f64])))
        (add s p)
    })
    .unwrap();
    // s = 3+4 = 7, p = 3*4 = 12, result = 7+12 = 19
    assert_dag_output_f64(run_dag_single(&wf), &[19.0], 1);
}

// Nested function mixed with outer workflow inputs.
#[test]
fn t_dag_nested_fn_with_outer_input() {
    let mut wf = Workflow::default();
    orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (fn double
            (add 'x 'x))
        (double 'val)
    })
    .unwrap();
    // Outer workflow has input 'val. Nested fn doubles it.
    let wf_ins = wf.workflow_inputs().unwrap();
    assert_eq!(wf_ins.len(), 1);
    assert_eq!(wf_ins[0].2, "val");
    let h = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let v (const [5.0f64, 7.0]))
        (return v)
    })
    .unwrap();
    let inputs = [h.borrowed()];
    let mut out = [OrcHandle::default()];
    wf.run(&inputs, &mut out, &host_clone_orc_handle, &HANDLE_COUNTER)
        .unwrap();
    let view = DeckView::<f64>::from_handle(&out[0]).unwrap();
    assert_eq!(view.items(), &[10.0, 14.0]);
}

// ==================== Full round-trip serialization ====================

/// Look up the plugin that owns `type_id`, matching the host's dispatch pattern.
fn plugin_for_type(type_id: OrcTypeId) -> &'static Plugin {
    match PLUGIN_SET.get_type_owner(type_id) {
        Some(TypeOwner::Plugin(plugin_index, _)) => &PLUGIN_SET.plugins()[*plugin_index],
        // Builtin types can be serialized by any plugin.
        Some(TypeOwner::BuiltIn(_)) => &PLUGIN_SET.plugins()[0],
        None => panic!("No plugin found for type_id {type_id}"),
    }
}

fn host_serialize(handle: &OrcHandle) -> Vec<u8> {
    plugin_for_type(handle.type_id)
        .serialize_deck(&SERIAL_CONTEXT_ARENA, handle, |buf| buf.clone())
        .expect("serialization failed")
}

fn host_deserialize(buf: &[u8], type_id: OrcTypeId) -> OrcHandle {
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    plugin_for_type(type_id)
        .deserialize_deck(0, buf, &mut out)
        .expect("deserialization failed");
    out
}

fn serial_round_trip(handle: &OrcHandle) -> OrcHandle {
    let buf = host_serialize(handle);
    host_deserialize(&buf, handle.type_id)
}

fn make_handle<T: orc_sdk::TOrcData>(deck: &Deck<T>) -> OrcHandle {
    let mut h = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    unsafe { update_handle_from_deck(deck, &mut h) };
    h
}

#[test]
fn t_serial_round_trip_f64_flat() {
    let d = deck![1.0_f64, 2.0, 3.0];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<f64>(), &[1.0, 2.0, 3.0]);
}

#[test]
fn t_serial_round_trip_f64_nested() {
    let d: Deck<f64> = deck![[1.0, 2.0], [3.0]];
    let h = make_handle(&d);
    assert!(h.n_marks > 0);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<f64>(), &[1.0, 2.0, 3.0]);
    assert_eq!(out.n_marks, h.n_marks);
    let orig_marks = unsafe { std::slice::from_raw_parts(h.marks, h.n_marks as usize) };
    let out_marks = unsafe { std::slice::from_raw_parts(out.marks, out.n_marks as usize) };
    for (a, b) in orig_marks.iter().zip(out_marks.iter()) {
        assert_eq!(a.depth, b.depth);
        assert_eq!(a.pos, b.pos);
    }
}

#[test]
fn t_serial_round_trip_f64_deeply_nested() {
    let d: Deck<f64> = deck![[[1.0, 2.0], [3.0]], [[4.0]]];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    let orig = DeckView::<f64>::from_handle(&h).unwrap();
    let restored = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(orig.depth(), restored.depth());
    assert_eq!(orig.items(), restored.items());
    assert_eq!(orig.marks().len(), restored.marks().len());
}

#[test]
fn t_serial_round_trip_i32() {
    let d = deck![10_i32, 20, 30, 40];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<i32>(), &[10, 20, 30, 40]);
}

#[test]
fn t_serial_round_trip_u8() {
    let d = deck![255_u8, 0, 128];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<u8>(), &[255, 0, 128]);
}

#[test]
fn t_serial_round_trip_u16() {
    let d = deck![100_u16, 200, 65535];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<u16>(), &[100, 200, 65535]);
}

#[test]
fn t_serial_round_trip_u32() {
    let d = deck![1000_u32, 2000, u32::MAX];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<u32>(), &[1000, 2000, u32::MAX]);
}

#[test]
fn t_serial_round_trip_u64() {
    let d = deck![u64::MAX, 0_u64, 42];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<u64>(), &[u64::MAX, 0, 42]);
}

#[test]
fn t_serial_round_trip_i8() {
    let d = deck![-128_i8, 0, 127];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<i8>(), &[-128, 0, 127]);
}

#[test]
fn t_serial_round_trip_i16() {
    let d = deck![-100_i16, 0, 100];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<i16>(), &[-100, 0, 100]);
}

#[test]
fn t_serial_round_trip_i64() {
    let d = deck![i64::MIN, 0_i64, i64::MAX];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<i64>(), &[i64::MIN, 0, i64::MAX]);
}

#[test]
fn t_serial_round_trip_f32() {
    let d = deck![1.5_f32, -2.5, 0.0];
    let h = make_handle(&d);
    let out = serial_round_trip(&h);
    assert_eq!(out.items::<f32>(), &[1.5f32, -2.5, 0.0]);
}

#[test]
fn t_serial_round_trip_empty_deck() {
    let d: Deck<f64> = Deck::default();
    let h = make_handle(&d);
    assert_eq!(h.n_items, 0);
    let out = serial_round_trip(&h);
    assert_eq!(out.n_items, 0);
}

#[test]
fn t_serial_round_trip_preserves_dims() {
    let d = deck![1.0_f64, 2.0];
    let mut h = make_handle(&d);
    h.dims[0] = 1;
    h.dims[1] = -2;
    h.dims[3] = 3;
    let out = serial_round_trip(&h);
    assert_eq!(out.dims, h.dims);
}

#[test]
fn t_serial_deserialize_trailing_bytes_fails() {
    let d = deck![42_i64];
    let h = make_handle(&d);
    let mut buf = host_serialize(&h);
    buf.push(0xFF);
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let result = plugin_for_type(h.type_id).deserialize_deck(0, &buf, &mut out);
    assert!(result.is_err());
}

#[test]
fn t_serial_deserialize_truncated_fails() {
    let d = deck![1.0_f64, 2.0, 3.0];
    let h = make_handle(&d);
    let buf = host_serialize(&h);
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let result = plugin_for_type(h.type_id).deserialize_deck(0, &buf[..buf.len() - 1], &mut out);
    assert!(result.is_err());
}

#[test]
fn t_serial_deserialize_empty_buffer_fails() {
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let result = plugin_for_type(orc_sdk::ORC_TYPE_F64).deserialize_deck(0, &[], &mut out);
    assert!(result.is_err());
}

/// Helper: call create_complex(real, imag) to produce a complex-typed handle.
fn create_complex_handle(reals: &[f64], imags: &[f64]) -> OrcHandle {
    let create_complex = PLUGIN_SET
        .get_function("create_complex")
        .expect("create_complex not found");
    let real_deck: Deck<f64> = Deck::from_raw_data(reals, &[]);
    let imag_deck: Deck<f64> = Deck::from_raw_data(imags, &[]);
    let real_h = make_handle(&real_deck);
    let imag_h = make_handle(&imag_deck);
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [real_h, imag_h];
    unsafe {
        (create_complex.func.expect("Invalid function"))(0, inputs.as_ptr(), 2, &mut out, 1);
    }
    out
}

/// Helper: call complex_get_parts to extract (reals, imags) from a complex handle.
fn extract_complex_parts(handle: &OrcHandle) -> (Vec<f64>, Vec<f64>) {
    let get_parts = PLUGIN_SET
        .get_function("complex_get_parts")
        .expect("complex_get_parts not found");
    let real_out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let imag_out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [handle.borrowed()];
    let mut outputs = [real_out, imag_out];
    unsafe {
        (get_parts.func.expect("Invalid function"))(
            0,
            inputs.as_ptr().cast(),
            1,
            outputs.as_mut_ptr(),
            2,
        );
    }
    let reals = outputs[0].items::<f64>().to_vec();
    let imags = outputs[1].items::<f64>().to_vec();
    (reals, imags)
}

#[test]
fn t_serial_round_trip_complex_flat() {
    let h = create_complex_handle(&[1.0, -3.5], &[2.0, 0.0]);
    let out = serial_round_trip(&h);
    assert_eq!(out.n_items, 2);
    assert_eq!(out.type_id, h.type_id);
    let (orig_re, orig_im) = extract_complex_parts(&h);
    let (out_re, out_im) = extract_complex_parts(&out);
    assert_eq!(orig_re, out_re);
    assert_eq!(orig_im, out_im);
}

#[test]
fn t_serial_round_trip_complex_nested() {
    // Create flat complex numbers, then nest them via a proxy.
    let create_complex = PLUGIN_SET
        .get_function("create_complex")
        .expect("create_complex not found");
    let real_deck: Deck<f64> = deck![[1.0, 2.0], [3.0]];
    let imag_deck: Deck<f64> = deck![[0.5, -0.5], [1.0]];
    let real_h = make_handle(&real_deck);
    let imag_h = make_handle(&imag_deck);
    let mut h = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    let inputs = [real_h, imag_h];
    unsafe {
        (create_complex.func.expect("Invalid function"))(0, inputs.as_ptr(), 2, &mut h, 1);
    }
    assert!(h.n_marks > 0, "expected nested complex deck");
    let buf = host_serialize(&h);
    let out = host_deserialize(&buf, h.type_id);
    assert_eq!(out.n_items, h.n_items);
    assert_eq!(out.n_marks, h.n_marks);
    let (orig_re, orig_im) = extract_complex_parts(&h);
    let (out_re, out_im) = extract_complex_parts(&out);
    assert_eq!(orig_re, out_re);
    assert_eq!(orig_im, out_im);
}

// ==================== Every plugin handles builtin types ====================

/// Serialize a handle using a specific plugin, and deserialize using another.
fn cross_plugin_round_trip(
    handle: &OrcHandle,
    serialize_plugin: &Plugin,
    deserialize_plugin: &Plugin,
) -> OrcHandle {
    let buf = serialize_plugin
        .serialize_deck(&SERIAL_CONTEXT_ARENA, handle, |buf| buf.clone())
        .expect("serialization failed");
    let mut out = OrcHandle {
        handle: next_id(),
        ..Default::default()
    };
    deserialize_plugin
        .deserialize_deck(0, &buf, &mut out)
        .expect("deserialization failed");
    out
}

#[test]
fn t_serial_every_plugin_handles_builtin_types() {
    let plugins = PLUGIN_SET.plugins();
    assert!(
        plugins.len() >= 2,
        "need at least 2 plugins to test cross-plugin builtin serialization"
    );
    macro_rules! test_type {
        ($ty:ty, [$($v:expr),+ $(,)?]) => {{
            let d: Deck<$ty> = deck![$($v),+];
            let h = make_handle(&d);
            let expected: &[$ty] = &[$($v),+];
            for (si, sp) in plugins.iter().enumerate() {
                for (di, dp) in plugins.iter().enumerate() {
                    let out = cross_plugin_round_trip(&h, sp, dp);
                    assert_eq!(
                        out.items::<$ty>(), expected,
                        "failed: serialize with plugin {si} ({}), deserialize with plugin {di} ({}), type {}",
                        sp.name(), dp.name(), stringify!($ty)
                    );
                }
            }
        }};
    }
    test_type!(u8, [1, 2, 3]);
    test_type!(u16, [100, 200, 300]);
    test_type!(u32, [1000, 2000]);
    test_type!(u64, [10000, 20000]);
    test_type!(i8, [-1, 0, 1]);
    test_type!(i16, [-100, 0, 100]);
    test_type!(i32, [-1000, 0, 1000]);
    test_type!(i64, [-10000, 0, 10000]);
    test_type!(f32, [1.5, -2.5, 0.0]);
    test_type!(f64, [1.5, -2.5, 0.0]);
}

#[test]
fn t_serial_every_plugin_handles_nested_builtin() {
    let plugins = PLUGIN_SET.plugins();
    let d: Deck<f64> = deck![[1.0, 2.0], [3.0]];
    let h = make_handle(&d);
    assert!(h.n_marks > 0);
    for (si, sp) in plugins.iter().enumerate() {
        for (di, dp) in plugins.iter().enumerate() {
            let out = cross_plugin_round_trip(&h, sp, dp);
            assert_eq!(
                out.items::<f64>(),
                &[1.0, 2.0, 3.0],
                "items mismatch: plugin {si} ({}) -> plugin {di} ({})",
                sp.name(),
                dp.name()
            );
            assert_eq!(out.n_marks, h.n_marks);
        }
    }
}
