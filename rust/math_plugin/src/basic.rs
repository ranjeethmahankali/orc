use crate::{host_callbacks, registry};
use orc_sdk::{DeckWriter, Error, HostCallbacks, OrcDims, TOrcData, orc_fn, orc_map_fn};
use std::ops::{Add, Div, Mul, Sub};

#[orc_fn]
fn add() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();
    let types = (run::<f64>, run::<i64>);

    /// Adds two inputs values, assigns result to the output. This function supports any integer or
    /// floating point scalar types. The two inputs must be of the same type. The output produced
    /// will be of the same type also.
    fn run<T>(lhs: &T, rhs: &T, out: &mut T)
    where
        T: TOrcData + Add<Output = T> + Copy,
    {
        *out = *lhs + *rhs;
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
}

#[orc_fn]
fn mul() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();
    let types = (run::<f64>, run::<i64>);

    /// Multiplies two inputs values, and assigns the result to the output. This function supports
    /// any integer or floating point scalar types. The two inputs must be of the same type. The
    /// output produced will be of the same type also.
    fn run<T>(lhs: &T, rhs: &T, out: &mut T)
    where
        T: TOrcData + Mul<Output = T> + Copy,
    {
        *out = *lhs * *rhs;
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
}

#[orc_fn]
fn sub() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();
    let types = (run::<f32>, run::<f64>);

    /// Subtracts the second operand from the first, and assigns to the output. The input types must
    /// be the same, matching the output type. This function supports floating point scalar types.
    fn run<T>(lhs: &T, rhs: &T, out: &mut T)
    where
        T: TOrcData + Sub<Output = T> + Copy,
    {
        *out = *lhs - *rhs;
    }
}

#[orc_fn]
fn div() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();
    let types = (run::<f32>, run::<f64>);

    /// Divides the first input with the second input, and assign to the output. All inputs must be
    /// of the same type, matching the output type. This function supports floating point scalar
    /// types.
    fn run<T>(lhs: &T, rhs: &T, out: &mut T)
    where
        T: TOrcData + Div<Output = T> + Copy,
    {
        *out = *lhs / *rhs;
    }
}

#[orc_fn]
fn pow() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(lhs: &f64, rhs: &f64, out: &mut f64) {
        *out = lhs.powf(*rhs);
    }
}

#[orc_fn]
fn repeat_list() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();
    let types = (
        run::<f32>, run::<f64>, run::<u8>, run::<u16>, run::<u32>, run::<u64>, run::<i8>,
        run::<i16>, run::<i32>, run::<i64>,
    );

    const OUTPUT_DEPTHS: [u8; 1] = [1u8];
    fn run<T>(
        _host: &HostCallbacks,
        list: &[T],
        repeat_count: &u64,
        out: &mut DeckWriter<T>,
    ) -> Result<(), Error>
    where
        T: TOrcData + Sub<Output = T> + Copy,
    {
        for _ in 0..(*repeat_count as usize) {
            out.extend_from_slice(list);
        }
        Ok(())
    }
}

#[orc_map_fn]
fn sin() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(lhs: &f64, rhs: &mut f64) {
        *rhs = lhs.sin();
    }

    fn dims(lhs: &OrcDims, out: &mut OrcDims) {
        *out = *lhs;
    }
}

#[orc_map_fn]
fn cos() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(lhs: &f64, rhs: &mut f64) {
        *rhs = lhs.cos();
    }
}

#[orc_map_fn]
fn tan() {
    let host_callbacks = host_callbacks();
    let registry: &ObjectRegistry = registry();

    fn run(lhs: &f64, rhs: &mut f64) {
        *rhs = lhs.tan();
    }
}

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
    fn t_add_f64_elementwise() {
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
    fn t_add_integers() {
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
    fn t_add_broadcast_scalar_to_list() {
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
    fn t_add_mismatched_types() {
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
    fn t_add_mismatched_dimensions() {
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
    fn t_add_wrong_n_inputs() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let mut out = out_handle();
        let inputs = [view(&lhs)]; // 1 instead of 2
        unsafe { add(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_add_output_reuse_same_type() {
        let lhs1: Deck<f64> = orc_sdk::deck![1.0];
        let rhs1: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();

        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(DeckView::<f64>::from_handle(&out).unwrap().items(), &[3.0]);
        let ptr_after_first = out.items;

        let lhs2: Deck<f64> = orc_sdk::deck![10.0];
        let rhs2: Deck<f64> = orc_sdk::deck![20.0];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        // Second call reuses the same registry slot.
        assert_eq!(DeckView::<f64>::from_handle(&out).unwrap().items(), &[30.0]);
        assert_eq!(out.handle, out.handle); // id unchanged (trivially)
        // Same output size — Vec capacity is sufficient, buffer address unchanged.
        assert_eq!(out.items, ptr_after_first);
    }

    #[test]
    fn t_add_output_type_change() {
        let mut out = out_handle();
        let out_id = out.handle;

        // First call: f64.
        let a: Deck<f64> = orc_sdk::deck![1.0];
        let b: Deck<f64> = orc_sdk::deck![2.0];
        let inputs1 = [view(&a), view(&b)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);

        // Second call: f32 — type changes cleanly.
        let c: Deck<i64> = orc_sdk::deck![3i64];
        let d: Deck<i64> = orc_sdk::deck![4i64];
        let inputs2 = [view(&c), view(&d)];
        unsafe { mul(0, inputs2.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, i64::TYPE_INFO.type_id);
        assert_eq!(out.handle, out_id);
        assert_eq!(
            DeckView::<i64>::from_handle(&out).unwrap().items(),
            &[12i64]
        );
    }

    // ==================== mul ====================

    #[test]
    fn t_mul_f64_elementwise() {
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
    fn t_mul_integer_types() {
        let lhs: Deck<i64> = orc_sdk::deck![3i64, 4];
        let rhs: Deck<i64> = orc_sdk::deck![7i64, 8];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { mul(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(
            DeckView::<i64>::from_handle(&out).unwrap().items(),
            &[21i64, 32]
        );
    }

    #[test]
    fn t_mul_mismatched_types() {
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
    fn t_sub_f64() {
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
    fn t_sub_unsupported_type() {
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
    fn t_div_f64() {
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
    fn t_div_by_zero() {
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
    fn t_div_unsupported_type() {
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
    fn t_add_reuse_same_type_handle_updated() {
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
    fn t_add_type_change_handle_updated() {
        // First call: f64.
        let lhs1: Deck<f64> = orc_sdk::deck![5.0];
        let rhs1: Deck<f64> = orc_sdk::deck![3.0];
        let mut out = out_handle();
        let out_id = out.handle;
        let inputs1 = [view(&lhs1), view(&rhs1)];
        unsafe { add(0, inputs1.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, f64::TYPE_INFO.type_id);

        // Second call: i32 — old deck freed, new deck allocated.
        let lhs2: Deck<i64> = orc_sdk::deck![100i64];
        let rhs2: Deck<i64> = orc_sdk::deck![200i64];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        // Handle must reflect the new type and data.
        assert_eq!(out.type_id, i64::TYPE_INFO.type_id);
        assert_eq!(out.handle, out_id);
        assert_eq!(out.n_items, 1);
        assert_eq!(
            DeckView::<i64>::from_handle(&out).unwrap().items(),
            &[300i64]
        );
    }

    #[test]
    fn t_add_clears_previous_data() {
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
        let ptr_after_first = out.items;
        // Second call: 1-item output — previous data must not bleed through.
        let lhs2: Deck<f64> = orc_sdk::deck![1.0];
        let rhs2: Deck<f64> = orc_sdk::deck![2.0];
        let inputs2 = [view(&lhs2), view(&rhs2)];
        unsafe { add(0, inputs2.as_ptr(), 2, &mut out, 1) };
        let result = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(result.items().len(), 1);
        assert_eq!(result.items(), &[3.0]);
        // Writing fewer items than the first call — Vec capacity is sufficient,
        // so no reallocation should occur: the buffer address is the same.
        assert_eq!(out.items, ptr_after_first);
    }

    // ==================== handle lifetime invariants ====================

    #[test]
    fn t_output_free_fn_set_after_call() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    #[test]
    fn t_output_handle_id_preserved() {
        let lhs: Deck<f64> = orc_sdk::deck![1.0];
        let rhs: Deck<f64> = orc_sdk::deck![2.0];
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&lhs), view(&rhs)];
        unsafe { add(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_output_owned_by_plugin_registry() {
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
    fn t_input_handles_unaffected() {
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

    // ==================== repeat_list ====================

    #[test]
    fn t_repeat_list_f64_basic() {
        let list: Deck<f64> = orc_sdk::deck![1.0, 2.0, 3.0];
        let count: Deck<u64> = orc_sdk::deck![3u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn t_repeat_list_u8() {
        let list: Deck<u8> = orc_sdk::deck![10u8, 20];
        let count: Deck<u64> = orc_sdk::deck![2u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<u8>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[10u8, 20, 10, 20]);
    }

    #[test]
    fn t_repeat_list_i32() {
        let list: Deck<i32> = orc_sdk::deck![-1i32, 0, 1];
        let count: Deck<u64> = orc_sdk::deck![2u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<i32>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[-1i32, 0, 1, -1, 0, 1]);
    }

    #[test]
    fn t_repeat_list_single_element() {
        let list: Deck<f64> = orc_sdk::deck![42.0];
        let count: Deck<u64> = orc_sdk::deck![4u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[42.0, 42.0, 42.0, 42.0]);
    }

    #[test]
    fn t_repeat_list_zero_repeats() {
        let list: Deck<f64> = orc_sdk::deck![1.0, 2.0];
        let count: Deck<u64> = orc_sdk::deck![0u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[] as &[f64]);
    }

    #[test]
    fn t_repeat_list_one_repeat() {
        let list: Deck<f64> = orc_sdk::deck![5.0, 6.0];
        let count: Deck<u64> = orc_sdk::deck![1u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<f64>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[5.0, 6.0]);
    }

    #[test]
    fn t_repeat_list_output_has_marks() {
        // OUTPUT_DEPTHS = [1], so the output should be a list (depth 1).
        let list: Deck<f64> = orc_sdk::deck![1.0, 2.0];
        let count: Deck<u64> = orc_sdk::deck![2u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.n_marks > 0);
        assert!(!out.marks.is_null());
    }

    #[test]
    fn t_repeat_list_output_free_fn_set() {
        let list: Deck<f64> = orc_sdk::deck![1.0];
        let count: Deck<u64> = orc_sdk::deck![1u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert!(out.free_fn.is_some());
    }

    #[test]
    fn t_repeat_list_output_handle_id_preserved() {
        let list: Deck<f64> = orc_sdk::deck![1.0];
        let count: Deck<u64> = orc_sdk::deck![1u64];
        let mut out = out_handle();
        let expected_id = out.handle;
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.handle, expected_id);
    }

    #[test]
    fn t_repeat_list_output_type_matches_input() {
        let list: Deck<u32> = orc_sdk::deck![7u32];
        let count: Deck<u64> = orc_sdk::deck![2u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        assert_eq!(out.type_id, u32::TYPE_INFO.type_id);
        let dv = DeckView::<u32>::from_handle(&out).unwrap();
        assert_eq!(dv.items(), &[7u32, 7]);
    }

    #[test]
    fn t_repeat_list_wrong_n_inputs() {
        let list: Deck<f64> = orc_sdk::deck![1.0];
        let mut out = out_handle();
        let inputs = [view(&list)]; // 1 instead of 2
        unsafe { repeat_list(0, inputs.as_ptr(), 1, &mut out, 1) };
        assert!(out.free_fn.is_none());
        assert!(out.items.is_null());
    }

    #[test]
    fn t_repeat_list_nested_input() {
        // Input is [[1.0, 2.0], [3.0]] with repeat count 2.
        // The combinations iterate over each sublist, repeating each.
        let list: Deck<f64> = orc_sdk::deck![[1.0, 2.0], [3.0]];
        let count: Deck<u64> = orc_sdk::deck![2u64];
        let mut out = out_handle();
        let inputs = [view(&list), view(&count)];
        unsafe { repeat_list(0, inputs.as_ptr(), 2, &mut out, 1) };
        let dv = DeckView::<f64>::from_handle(&out).unwrap();
        // Each sublist repeated: [1,2,1,2] and [3,3]
        assert_eq!(dv.items(), &[1.0, 2.0, 1.0, 2.0, 3.0, 3.0]);
    }
}
