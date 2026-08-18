use crate::{HANDLE_COUNTER, PLUGIN_SET, REGISTRY};
use orc_sdk::{
    DagError, DagOutputData, Deck, DeckView, IH, OH, OrcHandle, Workflow, deck, orc_dag,
    orc_inline_dag, update_handle_from_deck,
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

fn run_dag_single(wf: &Workflow) -> DagOutputData {
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&[], &mut out, &HANDLE_COUNTER).unwrap();
    let [o] = out;
    o
}

fn assert_dag_output_f64(data: DagOutputData, expected: &[f64], expected_depth: u8) {
    match data {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(&h).unwrap();
            assert_eq!(view.items(), expected);
            assert_eq!(view.depth(), expected_depth);
        }
        DagOutputData::Constant(id) => {
            REGISTRY
                .with_refs(&[id], |refs| {
                    let deck = refs[0].downcast_ref::<Deck<f64>>().unwrap();
                    let view = deck.view(deck.max_depth());
                    assert_eq!(view.items(), expected);
                    assert_eq!(view.depth(), expected_depth);
                })
                .unwrap();
        }
    }
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 22.0, 33.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[111.0, 222.0, 333.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[49.0, 169.0], 1);
}

// 4. Nested single-output call.
#[test]
fn t_dag_nested_call() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add (mul (const [2.0f64, 3.0]) (const [5.0f64, 10.0])) (const [1.0f64, 1.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 31.0], 1);
}

// 5. Deeply nested calls.
#[test]
fn t_dag_deep_nesting() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (mul (add (const [1.0f64, 2.0]) (const [3.0f64, 4.0])) (const [10.0f64, 10.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&mut wf), &[40.0, 60.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[10.0], 0);
}

// 9. Const scalars used as inputs.
#[test]
fn t_dag_const_inputs() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (sub (const 100.0f64) (const 1.0f64))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&mut wf), &[99.0], 0);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[10.0], 0);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 22.0, 33.0], 1);
}

// 14. Const used inline as expression argument.
#[test]
fn t_dag_const_inline_expr() {
    let mut wf = Workflow::default();
    let _oh = orc_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, &mut wf, {
        (add (const [2.0f64, 4.0]) (const [10.0f64, 20.0]))
    })
    .unwrap();
    assert_dag_output_f64(run_dag_single(&mut wf), &[12.0, 24.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[6.0, 12.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 22.0, 33.0], 2);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 22.0, 33.0, 44.0], 3);
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
    let mut out = [DagOutputData::Constant(0), DagOutputData::Constant(0)];
    wf.run(&[], &mut out, &HANDLE_COUNTER).unwrap();
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[11.0, 22.0, 33.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[9.0, 16.0], 1);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[1.0, 2.0, 3.0], 1);
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
    let mut out = [DagOutputData::Constant(0)];
    let result = wf.run(&[], &mut out, &HANDLE_COUNTER);
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
    let mut out = [DagOutputData::Constant(0)];
    let result = wf.run(&[], &mut out, &HANDLE_COUNTER);
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
    assert_dag_output_f64(run_dag_single(&mut wf), &[8.0, 15.0], 1);
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
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&inputs, &mut out, &HANDLE_COUNTER).unwrap();
    match &out[0] {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(h).unwrap();
            assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
        }
        _ => panic!("Expected owned output"),
    }
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
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&inputs, &mut out, &HANDLE_COUNTER).unwrap();
    match &out[0] {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(h).unwrap();
            assert_eq!(view.items(), &[9.0, 16.0]);
        }
        _ => panic!("Expected owned output"),
    }
    // Second run: x = [5, 6]
    let h2 = orc_inline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, &*REGISTRY, {
        (let v (const [5.0f64, 6.0]))
        (return v)
    })
    .unwrap();
    let inputs = [h2.borrowed()];
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&inputs, &mut out, &HANDLE_COUNTER).unwrap();
    match &out[0] {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(h).unwrap();
            assert_eq!(view.items(), &[25.0, 36.0]);
        }
        _ => panic!("Expected owned output"),
    }
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
    let mut out = [DagOutputData::Constant(0), DagOutputData::Constant(0)];
    wf.run(&[], &mut out, &HANDLE_COUNTER).unwrap();
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
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&inputs, &mut out, &HANDLE_COUNTER).unwrap();
    match &out[0] {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(h).unwrap();
            assert_eq!(view.items(), &[11.0, 22.0, 33.0]);
        }
        _ => panic!("Expected owned output"),
    }
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
    let mut out = [DagOutputData::Constant(0)];
    wf.run(&inputs, &mut out, &HANDLE_COUNTER).unwrap();
    match &out[0] {
        DagOutputData::Owned(h) => {
            let view = DeckView::<f64>::from_handle(h).unwrap();
            assert_eq!(view.items(), &[9.0, 16.0]);
        }
        _ => panic!("Expected owned output"),
    }
}
