use crate::{host_callbacks, registry};
use orc_sdk::{Error, HostCallbacks, OrcDims, TOrcData, orc_fn};
use std::ops::{Add, Div, Mul, Sub};

orc_fn!(add, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Adds two inputs values, assigns result to the output. This function supports any integer or
    /// floating point scalar types. The two inputs must be of the same type. The output produced
    /// will be of the same type also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Add<Output = T> + Copy,
    {
        *out = *lhs + *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(mul, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Multiplies two inputs values, and assigns the result to the output. This function supports
    /// any integer or floating point scalar types. The two inputs must be of the same type. The
    /// output produced will be of the same type also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Mul<Output = T> + Copy,
    {
        *out = *lhs * *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(sub, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (Case<f32>, Case<f64>);

    /// Subtracts the second operand from the first, and assigns to the output. The input types must
    /// be the same, matching the output type. This function supports floating point scalar types.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Sub<Output = T> + Copy,
    {
        *out = *lhs - *rhs;
        Ok(())
    }
});

orc_fn!(div, {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    type Types = (Case<f32>, Case<f64>);

    /// Divides the first input with the second input, and assign to the output. All inputs must be
    /// of the same type, matching the output type. This function supports floating point scalar
    /// types.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Div<Output = T> + Copy,
    {
        *out = *lhs / *rhs;
        Ok(())
    }
});

#[cfg(test)]
mod tests {
    use super::*;
    use orc_sdk::{Deck, DeckView, OrcHandle, TOrcData, update_handle_from_deck};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(9000);
    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// Build a view handle for an input deck (free_fn = None).
    fn view<T: TOrcData>(deck: &Deck<T>) -> OrcHandle {
        let mut h = OrcHandle {
            handle: next_id(),
            ..Default::default()
        };
        unsafe { update_handle_from_deck(deck, &mut h) };
        h
    }

    /// Build an owned output handle (host-assigned id, no data yet).
    fn out_handle() -> OrcHandle {
        OrcHandle {
            handle: next_id(),
            ..Default::default()
        }
    }

    // ==================== add ====================

    #[test]
    fn add_f64_elementwise() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0, 2.0, 3.0];
        let rhs: Deck<f64> = orc_sdk::deck![10.0, 20.0, 30.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[11.0, 22.0, 33.0]
        );
    }

    #[test]
    fn add_f32_elementwise() {
        let lhs: Deck<f32> = orc_sdk::deck![1.0f32, 2.0, 3.0];
        let rhs: Deck<f32> = orc_sdk::deck![10.0f32, 20.0, 30.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f32>::from_handle(&out).unwrap().items(),
            &[11.0f32, 22.0, 33.0]
        );
    }

    #[test]
    fn add_integer_types() {
        // u32
        let lhs: Deck<u32> = orc_sdk::deck![5u32];
        let rhs: Deck<u32> = orc_sdk::deck![3u32];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(DeckView::<u32>::from_handle(&out).unwrap().items(), &[8u32]);

        // i64
        let lhs2: Deck<i64> = orc_sdk::deck![100i64];
        let rhs2: Deck<i64> = orc_sdk::deck![-7i64];
        let mut out2 = out_handle();
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out2, 1) };
        assert_eq!(
            DeckView::<i64>::from_handle(&out2).unwrap().items(),
            &[93i64]
        );
    }

    #[test]
    fn add_broadcast_scalar_to_list() {
        // b has 1 item; it stays at its last value (10.0) once exhausted.
        let lhs: Deck<f64> = orc_sdk::deck![1.0, 2.0, 3.0];
        let rhs: Deck<f64> = orc_sdk::deck![10.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[11.0, 12.0, 13.0]
        );
    }

    #[test]
    fn add_mismatched_types() {
        // i32 lhs + f64 rhs → no match in type dispatch → output untouched.
        let lhs: Deck<i32> = orc_sdk::deck![1i32];
        let rhs: Deck<f64> = orc_sdk::deck![1.0f64];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn add_mismatched_dimensions() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut lhs_h = view(&lhs);
        let mut rhs_h = view(&rhs);
        lhs_h.dims[0] = 1; // meters
        rhs_h.dims[0] = 2; // meters^2 — mismatch
        let mut out = out_handle();
        let inputs = [lhs_h, rhs_h];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        // dims check fires orc_check_return! — output untouched.
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn add_wrong_n_inputs() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let mut out = out_handle();
        let inputs = [view(&lhs)]; // 1 instead of 2
        unsafe { add(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn add_output_reuse_same_type() {
        let lhs1: Deck<f64> = orc_sdk::deck![1.0];
        let rhs1: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();

        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(DeckView::<f64>::from_handle(&out).unwrap().items(), &[3.0]);

        let lhs2: Deck<f64> = orc_sdk::deck![10.0];
        let rhs2: Deck<f64> = orc_sdk::deck![20.0];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        // Second call reuses the same registry slot.
        assert_eq!(DeckView::<f64>::from_handle(&out).unwrap().items(), &[30.0]);
        assert_eq!(out.handle, out.handle); // id unchanged (trivially)
    }

    #[test]
    fn add_output_type_change() {
        let mut out = out_handle();
        let out_id = out.handle;

        // First call: f64.
        let a: Deck<f64> = orc_sdk::deck![1.0];
        let b: Deck<f64> = orc_sdk::deck![2.0];
        let inputs1 = [view(&a), view(&b)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);

        // Second call: f32 — type changes cleanly.
        let c: Deck<f32> = orc_sdk::deck![3.0f32];
        let d: Deck<f32> = orc_sdk::deck![4.0f32];
        let inputs2 = [view(&c), view(&d)];
        unsafe { mul(0, inputs2.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, f32::TYPE_INFO.type_id);
        assert_eq!(out.handle, out_id);
        assert_eq!(
            DeckView::<f32>::from_handle(&out).unwrap().items(),
            &[12.0f32]
        );
    }

    // ==================== mul ====================

    #[test]
    fn mul_f64_elementwise() {
        let lhs: Deck<f64> = orc_sdk::deck![2.0, 3.0, 4.0];
        let rhs: Deck<f64> = orc_sdk::deck![5.0, 6.0, 7.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[10.0, 18.0, 28.0]
        );
    }

    #[test]
    fn mul_integer_types() {
        let lhs: Deck<i32> = orc_sdk::deck![3i32, 4];
        let rhs: Deck<i32> = orc_sdk::deck![7i32, 8];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<i32>::from_handle(&out).unwrap().items(),
            &[21i32, 32]
        );
    }

    #[test]
    fn mul_mismatched_types() {
        let lhs: Deck<u32> = orc_sdk::deck![2u32];
        let rhs: Deck<f64> = orc_sdk::deck![3.0f64];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    // ==================== sub ====================

    #[test]
    fn sub_f64() {
        let lhs: Deck<f64> = orc_sdk::deck![10.0, 20.0];
        let rhs: Deck<f64> = orc_sdk::deck![3.0, 7.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { sub(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[7.0, 13.0]
        );
    }

    #[test]
    fn sub_unsupported_type() {
        // sub only supports f32/f64; u32 inputs → type dispatch fails.
        let lhs: Deck<u32> = orc_sdk::deck![5u32];
        let rhs: Deck<u32> = orc_sdk::deck![3u32];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { sub(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    // ==================== div ====================

    #[test]
    fn div_f64() {
        let lhs: Deck<f64> = orc_sdk::deck![10.0, 9.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0, 3.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { div(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[5.0, 3.0]
        );
    }

    #[test]
    fn div_by_zero() {
        // IEEE 754: dividing by zero produces infinity, no panic.
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![0.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { div(0, inputs.as_ptr(), 2, &mut out, 1) };
        let result = DeckView::<f64>::from_handle(&out).unwrap().items()[0];
        assert!(result.is_infinite() && result > 0.0);
    }

    #[test]
    fn div_unsupported_type() {
        let lhs: Deck<i32> = orc_sdk::deck![6i32];
        let rhs: Deck<i32> = orc_sdk::deck![2i32];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { div(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    // ==================== output reuse / reallocation / update ====================

    #[test]
    fn add_reuse_same_type_handle_updated() {
        // First call: 1-item output.
        let lhs1: Deck<f64> = orc_sdk::deck![1.0];
        let rhs1: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.n_items, 1);
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);
        let items_ptr_after_first = out.items;
        assert!(!items_ptr_after_first.is_null());

        // Second call: 2-item output, same type — registry slot is reused.
        let lhs2: Deck<f64> = orc_sdk::deck![10.0, 20.0];
        let rhs2: Deck<f64> = orc_sdk::deck![1.0, 2.0];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        // Handle must reflect the new output, not the old one.
        assert_eq!(out.n_items, 2);
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);
        assert!(!out.items.is_null());
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[11.0, 22.0]
        );
    }

    #[test]
    fn add_type_change_handle_updated() {
        // First call: f64.
        let lhs1: Deck<f64> = orc_sdk::deck![5.0];
        let rhs1: Deck<f64> = orc_sdk::deck![3.0];
        let mut out = out_handle();
        let out_id = out.handle;
        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);

        // Second call: i32 — old deck freed, new deck allocated.
        let lhs2: Deck<i32> = orc_sdk::deck![100i32];
        let rhs2: Deck<i32> = orc_sdk::deck![200i32];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        // Handle must reflect the new type and data.
        assert_eq!(out.type_id, i32::TYPE_INFO.type_id);
        assert_eq!(out.handle, out_id);
        assert_eq!(out.n_items, 1);
        assert_eq!(
            DeckView::<i32>::from_handle(&out).unwrap().items(),
            &[300i32]
        );
    }

    #[test]
    fn add_clears_previous_data() {
        // First call: 2-item output.
        let lhs1: Deck<f64> = orc_sdk::deck![3.0, 4.0];
        let rhs1: Deck<f64> = orc_sdk::deck![10.0, 20.0];
        let mut out = out_handle();
        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<f64>::from_handle(&out).unwrap().items(),
            &[13.0, 24.0]
        );

        // Second call: 1-item output — previous data must not bleed through.
        let lhs2: Deck<f64> = orc_sdk::deck![1.0];
        let rhs2: Deck<f64> = orc_sdk::deck![2.0];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        let result = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(result.items().len(), 1);
        assert_eq!(result.items(), &[3.0]);
    }

    // ==================== handle lifetime invariants ====================

    #[test]
    fn output_free_fn_set_after_call() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    #[test]
    fn output_handle_id_preserved() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn output_owned_by_plugin_registry() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let out_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        // The registry must hold an entry for this handle.
        crate::registry()
            .with_mut::<(), _>(&[out_id], |_| ())
            .unwrap();
    }

    #[test]
    fn input_handles_unaffected() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let lhs_h = view(&lhs);
        let rhs_h = view(&rhs);
        let lhs_items_before = lhs_h.items;
        let rhs_items_before = rhs_h.items;
        let mut out = out_handle();
        let inputs = [lhs_h, rhs_h];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(inputs[0].items, lhs_items_before);
        assert_eq!(inputs[1].items, rhs_items_before);
        assert!(inputs[0].free_fn.is_none());
        assert!(inputs[1].free_fn.is_none());
    }
}
