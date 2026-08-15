use crate::{HANDLE_COUNTER, PLUGIN_SET};
use orc_sdk::{Deck, DeckView, OrcHandle, deck, orc_iniline_dag, update_handle_from_deck};

use std::sync::atomic::{AtomicU64, Ordering};

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
// orc_dag macro tests.
// ==================================================

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
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
        (add a b)
    })
    .unwrap();
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
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
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
    let a_deck: Deck<f64> = deck![2.0, 3.0];
    let b_deck: Deck<f64> = deck![5.0, 10.0];
    let a = make_handle(&HANDLE_COUNTER, &a_deck);
    let b = make_handle(&HANDLE_COUNTER, &b_deck);
    // (a + b) * (a + b) = (7, 13) * (7, 13) = (49, 169)
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
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
    let a_deck: Deck<f64> = deck![2.0, 3.0];
    let b_deck: Deck<f64> = deck![5.0, 10.0];
    let c_deck: Deck<f64> = deck![1.0, 1.0];
    let a = make_handle(&HANDLE_COUNTER, &a_deck);
    let b = make_handle(&HANDLE_COUNTER, &b_deck);
    let c = make_handle(&HANDLE_COUNTER, &c_deck);
    // (a * b) + c = (10, 30) + (1, 1) = (11, 31)
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
        (add (mul a b) c)
    })
    .unwrap();
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
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
        (mul (add a b) c)
    })
    .unwrap();
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
    let out = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
        (let prod (mul a b))
        (sub (add prod c) prod)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&out).unwrap();
    assert_eq!(view.items(), &[10.0]);
}

// 9. External variables are usable as inputs.
#[test]
fn t_external_variables() {
    let x_deck: Deck<f64> = deck![100.0];
    let y_deck: Deck<f64> = deck![1.0];
    let x = make_handle(&HANDLE_COUNTER, &x_deck);
    let y = make_handle(&HANDLE_COUNTER, &y_deck);
    let result = orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {
        (sub x y)
    })
    .unwrap();
    let view = DeckView::<f64>::from_handle(&result).unwrap();
    assert_eq!(view.items(), &[99.0]);
}

// 10. Empty block returns unit.
#[test]
fn t_empty_block() {
    assert_eq!(orc_iniline_dag!(*PLUGIN_SET, &HANDLE_COUNTER, {}).unwrap(), ());
}
