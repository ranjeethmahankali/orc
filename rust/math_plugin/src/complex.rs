use crate::{host_callbacks, registry};
use orc_sdk::{Error, HostCallbacks, OrcTypeId, TOrcData, orc_fn};

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Complex {
    real: f64,
    imag: f64,
}

pub const COMPLEX_NUM_TYPE_ID: OrcTypeId = 0xd17d7399a9b11a54;

impl TOrcData for Complex {
    const TYPE_INFO: orc_sdk::OrcTypeInfo = orc_sdk::OrcTypeInfo {
        type_id: COMPLEX_NUM_TYPE_ID,
        name: c"complex number".as_ptr(),
        desc: c"3 + 2i type of stuff.".as_ptr(),
    };
}

orc_fn!(create_complex, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(_host: &HostCallbacks, real: &f64, imag: &f64, out: &mut Complex) -> Result<(), Error> {
        *out = Complex {
            real: *real,
            imag: *imag,
        };
        Ok(())
    }
});

orc_fn!(add_complex, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(
        _host: &HostCallbacks,
        lhs: &Complex,
        rhs: &Complex,
        out: &mut Complex,
    ) -> Result<(), Error> {
        *out = Complex {
            real: lhs.real + rhs.real,
            imag: lhs.imag + rhs.imag,
        };
        Ok(())
    }
});

orc_fn!(mul_complex, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(
        _host: &HostCallbacks,
        lhs: &Complex,
        rhs: &Complex,
        out: &mut Complex,
    ) -> Result<(), Error> {
        *out = Complex {
            real: lhs.real * rhs.real - lhs.imag * rhs.imag,
            imag: lhs.real * rhs.imag + rhs.real * lhs.imag,
        };
        Ok(())
    }
});

orc_fn!(complex_get_parts, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(
        _host: &HostCallbacks,
        c: &Complex,
        real_out: &mut f64,
        imag_out: &mut f64,
    ) -> Result<(), Error> {
        *real_out = c.real;
        *imag_out = c.imag;
        Ok(())
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use orc_sdk::{Deck, OrcHandle, TOrcData, deck, update_handle_from_deck};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(5000);

    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    fn handle<T: TOrcData>(deck: &Deck<T>) -> OrcHandle {
        let mut h = OrcHandle {
            handle: next_id(),
            ..Default::default()
        };
        unsafe { update_handle_from_deck(deck, &mut h) };
        h
    }

    fn out_handle() -> OrcHandle {
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        }
    }

    fn make_complex(real: &Deck<f64>, imag: &Deck<f64>) -> OrcHandle {
        let mut out = out_handle();
        let inputs = [handle(real), handle(imag)];
        unsafe { create_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        out
    }

    // ==================== add_complex ====================

    #[test]
    fn t_add_complex_flat_list() {
        // [1+2i, 3+4i] + [10+20i, 30+40i] = [11+22i, 33+44i]
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let rhs = make_complex(&deck![10.0, 30.0], &deck![20.0, 40.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: 11.0,
                    imag: 22.0
                },
                Complex {
                    real: 33.0,
                    imag: 44.0
                }
            ]
        );
    }

    #[test]
    fn t_add_complex_nested_lists() {
        // [[1+i, 2+i], [3+i]] + [[4+i, 5+i], [6+i]] = [[5+2i, 7+2i], [9+2i]]
        let lhs = make_complex(&deck![[1.0, 2.0], [3.0]], &deck![[1.0, 1.0], [1.0]]);
        let rhs = make_complex(&deck![[4.0, 5.0], [6.0]], &deck![[1.0, 1.0], [1.0]]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: 5.0,
                    imag: 2.0
                },
                Complex {
                    real: 7.0,
                    imag: 2.0
                },
                Complex {
                    real: 9.0,
                    imag: 2.0
                }
            ]
        );
        assert!(out.n_marks > 0);
    }

    #[test]
    fn t_add_complex_negative_components() {
        // [1-2i, -3+4i] + [0+3i, 3-4i] = [1+1i, 0+0i]
        let lhs = make_complex(&deck![1.0, -3.0], &deck![-2.0, 4.0]);
        let rhs = make_complex(&deck![0.0, 3.0], &deck![3.0, -4.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: 1.0,
                    imag: 1.0
                },
                Complex {
                    real: 0.0,
                    imag: 0.0
                }
            ]
        );
    }

    #[test]
    fn t_add_complex_wrong_n_inputs() {
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let mut out = out_handle();
        let inputs = [lhs];
        unsafe { add_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_add_complex_output_handle_id_preserved() {
        let lhs = make_complex(&deck![1.0, 0.0], &deck![0.0, 1.0]);
        let rhs = make_complex(&deck![0.0, 1.0], &deck![1.0, 0.0]);
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [lhs, rhs];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_add_complex_output_free_fn_set() {
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let rhs = make_complex(&deck![5.0, 7.0], &deck![6.0, 8.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    // ==================== mul_complex ====================

    #[test]
    fn t_mul_complex_flat_list() {
        // [1+2i, 2+3i] * [3+4i, 1+0i] = [-5+10i, 2+3i]
        let lhs = make_complex(&deck![1.0, 2.0], &deck![2.0, 3.0]);
        let rhs = make_complex(&deck![3.0, 1.0], &deck![4.0, 0.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: -5.0,
                    imag: 10.0
                },
                Complex {
                    real: 2.0,
                    imag: 3.0
                }
            ]
        );
    }

    #[test]
    fn t_mul_complex_nested_lists() {
        // [[1+0i, 0+1i], [2+0i]] * [[5+0i, 0+1i], [3+0i]] = [[5+0i, -1+0i], [6+0i]]
        let lhs = make_complex(&deck![[1.0, 0.0], [2.0]], &deck![[0.0, 1.0], [0.0]]);
        let rhs = make_complex(&deck![[5.0, 0.0], [3.0]], &deck![[0.0, 1.0], [0.0]]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: 5.0,
                    imag: 0.0
                },
                Complex {
                    real: -1.0,
                    imag: 0.0
                },
                Complex {
                    real: 6.0,
                    imag: 0.0
                },
            ]
        );
        assert!(out.n_marks > 0);
    }

    #[test]
    fn t_mul_complex_i_squared_list() {
        // [0+1i, 0+1i, 0+1i] * [0+1i, 0+1i, 0+1i] = [-1+0i, -1+0i, -1+0i]
        let lhs = make_complex(&deck![0.0, 0.0, 0.0], &deck![1.0, 1.0, 1.0]);
        let rhs = make_complex(&deck![0.0, 0.0, 0.0], &deck![1.0, 1.0, 1.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: -1.0,
                    imag: 0.0
                },
                Complex {
                    real: -1.0,
                    imag: 0.0
                },
                Complex {
                    real: -1.0,
                    imag: 0.0
                },
            ]
        );
    }

    #[test]
    fn t_mul_complex_by_zero_list() {
        // [3+4i, 1+1i] * [0+0i, 0+0i] = [0+0i, 0+0i]
        let lhs = make_complex(&deck![3.0, 1.0], &deck![4.0, 1.0]);
        let rhs = make_complex(&deck![0.0, 0.0], &deck![0.0, 0.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            out.items::<Complex>(),
            &[
                Complex {
                    real: 0.0,
                    imag: 0.0
                },
                Complex {
                    real: 0.0,
                    imag: 0.0
                }
            ]
        );
    }

    #[test]
    fn t_mul_complex_wrong_n_inputs() {
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let mut out = out_handle();
        let inputs = [lhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_mul_complex_output_handle_id_preserved() {
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let rhs = make_complex(&deck![5.0, 7.0], &deck![6.0, 8.0]);
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_mul_complex_output_free_fn_set() {
        let lhs = make_complex(&deck![1.0, 3.0], &deck![2.0, 4.0]);
        let rhs = make_complex(&deck![5.0, 7.0], &deck![6.0, 8.0]);
        let mut out = out_handle();
        let inputs = [lhs, rhs];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }
}
