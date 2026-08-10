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

    fn complex(real: f64, imag: f64) -> Deck<Complex> {
        orc_sdk::deck![Complex { real, imag }]
    }

    fn result(out: &OrcHandle) -> Complex {
        DeckView::<Complex>::from_handle(out).unwrap().items()[0].clone()
    }

    // ==================== add_complex ====================

    #[test]
    fn t_add_complex_basic() {
        // (1+2i) + (3+4i) = (4+6i)
        let lhs = complex(1.0, 2.0);
        let rhs = complex(3.0, 4.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, 4.0);
        assert_eq!(r.imag, 6.0);
    }

    #[test]
    fn t_add_complex_negative_imag() {
        // (1-2i) + (0+3i) = (1+1i)
        let lhs = complex(1.0, -2.0);
        let rhs = complex(0.0, 3.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, 1.0);
        assert_eq!(r.imag, 1.0);
    }

    #[test]
    fn t_add_complex_zero() {
        // (0+0i) + (5+5i) = (5+5i)
        let lhs = complex(0.0, 0.0);
        let rhs = complex(5.0, 5.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, 5.0);
        assert_eq!(r.imag, 5.0);
    }

    #[test]
    fn t_add_complex_wrong_n_inputs() {
        let lhs = complex(1.0, 2.0);
        let mut out = out_handle();
        let inputs = [view(&lhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_add_complex_output_handle_id_preserved() {
        let lhs = complex(1.0, 0.0);
        let rhs = complex(0.0, 1.0);
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_add_complex_output_free_fn_set() {
        let lhs = complex(1.0, 2.0);
        let rhs = complex(3.0, 4.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    // ==================== mul_complex ====================

    #[test]
    fn t_mul_complex_basic() {
        // (1+2i) * (3+4i) = (3-8) + (4+6)i = -5 + 10i
        let lhs = complex(1.0, 2.0);
        let rhs = complex(3.0, 4.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, -5.0);
        assert_eq!(r.imag, 10.0);
    }

    #[test]
    fn t_mul_complex_i_squared() {
        // (0+1i) * (0+1i) = -1 + 0i
        let lhs = complex(0.0, 1.0);
        let rhs = complex(0.0, 1.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, -1.0);
        assert_eq!(r.imag, 0.0);
    }

    #[test]
    fn t_mul_complex_by_real() {
        // (2+3i) * (5+0i) = (10+15i)
        let lhs = complex(2.0, 3.0);
        let rhs = complex(5.0, 0.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, 10.0);
        assert_eq!(r.imag, 15.0);
    }

    #[test]
    fn t_mul_complex_by_zero() {
        // (3+4i) * (0+0i) = (0+0i)
        let lhs = complex(3.0, 4.0);
        let rhs = complex(0.0, 0.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        let r = result(&out);
        assert_eq!(r.real, 0.0);
        assert_eq!(r.imag, 0.0);
    }

    #[test]
    fn t_mul_complex_wrong_n_inputs() {
        let lhs = complex(1.0, 2.0);
        let mut out = out_handle();
        let inputs = [view(&lhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_mul_complex_output_handle_id_preserved() {
        let lhs = complex(1.0, 2.0);
        let rhs = complex(3.0, 4.0);
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_mul_complex_output_free_fn_set() {
        let lhs = complex(1.0, 2.0);
        let rhs = complex(3.0, 4.0);
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul_complex(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }
}
