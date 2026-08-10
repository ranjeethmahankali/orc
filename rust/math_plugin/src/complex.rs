use crate::{host_callbacks, registry};
use orc_sdk::{Error, HostCallbacks, TOrcData, orc_fn};

#[derive(Default, Clone)]
struct Complex {
    real: f64,
    imag: f64,
}

impl TOrcData for Complex {
    const TYPE_INFO: orc_sdk::OrcTypeInfo = orc_sdk::OrcTypeInfo {
        type_id: 0xd17d7399a9b11a54,
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

#[cfg(test)]
mod tests {
    use super::*;
    use orc_sdk::{Deck, DeckView, OrcHandle, update_handle_from_deck};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(5000);
    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    fn view(deck: &Deck<Complex>) -> OrcHandle {
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

    fn complex(real: f64, imag: f64) -> Complex {
        Complex { real, imag }
    }

    fn items(out: &OrcHandle) -> Vec<(f64, f64)> {
        DeckView::<Complex>::from_handle(out)
            .unwrap()
            .items()
            .iter()
            .map(|c| (c.real, c.imag))
            .collect()
    }

    // ==================== add_complex ====================

    #[test]
    fn t_add_complex_flat_list() {
        // [1+2i, 3+4i] + [10+20i, 30+40i] = [11+22i, 33+44i]
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(10.0, 20.0), complex(30.0, 40.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(11.0, 22.0), (33.0, 44.0)]);
    }

    #[test]
    fn t_add_complex_nested_lists() {
        // [[1+i, 2+i], [3+i]] + [[4+i, 5+i], [6+i]] = [[5+2i, 7+2i], [9+2i]]
        let lhs: Deck<Complex> =
            orc_sdk::deck![[complex(1.0, 1.0), complex(2.0, 1.0)], [complex(3.0, 1.0)]];
        let rhs: Deck<Complex> =
            orc_sdk::deck![[complex(4.0, 1.0), complex(5.0, 1.0)], [complex(6.0, 1.0)]];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(5.0, 2.0), (7.0, 2.0), (9.0, 2.0)]);
        assert!(out.n_marks > 0);
    }

    #[test]
    fn t_add_complex_negative_components() {
        // [1-2i, -3+4i] + [0+3i, 3-4i] = [1+1i, 0+0i]
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, -2.0), complex(-3.0, 4.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(0.0, 3.0), complex(3.0, -4.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(1.0, 1.0), (0.0, 0.0)]);
    }

    #[test]
    fn t_add_complex_wrong_n_inputs() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_add_complex_output_handle_id_preserved() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 0.0), complex(0.0, 1.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(0.0, 1.0), complex(1.0, 0.0)];
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_add_complex_output_free_fn_set() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(5.0, 6.0), complex(7.0, 8.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    // ==================== mul_complex ====================

    #[test]
    fn t_mul_complex_flat_list() {
        // [1+2i, 2+3i] * [3+4i, 1+0i] = [-5+10i, 2+3i]
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(2.0, 3.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(3.0, 4.0), complex(1.0, 0.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(-5.0, 10.0), (2.0, 3.0)]);
    }

    #[test]
    fn t_mul_complex_nested_lists() {
        // [[1+0i, 0+1i], [2+0i]] * [[5+0i, 0+1i], [3+0i]] = [[5+0i, -1+0i], [6+0i]]
        let lhs: Deck<Complex> =
            orc_sdk::deck![[complex(1.0, 0.0), complex(0.0, 1.0)], [complex(2.0, 0.0)]];
        let rhs: Deck<Complex> =
            orc_sdk::deck![[complex(5.0, 0.0), complex(0.0, 1.0)], [complex(3.0, 0.0)]];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(5.0, 0.0), (-1.0, 0.0), (6.0, 0.0)]);
        assert!(out.n_marks > 0);
    }

    #[test]
    fn t_mul_complex_i_squared_list() {
        // [0+1i, 0+1i, 0+1i] * [0+1i, 0+1i, 0+1i] = [-1+0i, -1+0i, -1+0i]
        let lhs: Deck<Complex> =
            orc_sdk::deck![complex(0.0, 1.0), complex(0.0, 1.0), complex(0.0, 1.0)];
        let rhs: Deck<Complex> =
            orc_sdk::deck![complex(0.0, 1.0), complex(0.0, 1.0), complex(0.0, 1.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(-1.0, 0.0), (-1.0, 0.0), (-1.0, 0.0)]);
    }

    #[test]
    fn t_mul_complex_by_zero_list() {
        // [3+4i, 1+1i] * [0+0i, 0+0i] = [0+0i, 0+0i]
        let lhs: Deck<Complex> = orc_sdk::deck![complex(3.0, 4.0), complex(1.0, 1.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(0.0, 0.0), complex(0.0, 0.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(items(&out), vec![(0.0, 0.0), (0.0, 0.0)]);
    }

    #[test]
    fn t_mul_complex_wrong_n_inputs() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_mul_complex_output_handle_id_preserved() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(5.0, 6.0), complex(7.0, 8.0)];
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_mul_complex_output_free_fn_set() {
        let lhs: Deck<Complex> = orc_sdk::deck![complex(1.0, 2.0), complex(3.0, 4.0)];
        let rhs: Deck<Complex> = orc_sdk::deck![complex(5.0, 6.0), complex(7.0, 8.0)];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }
}
