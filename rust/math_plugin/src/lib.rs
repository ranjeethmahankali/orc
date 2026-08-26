use complex::Complex;
use orc_sdk::{
    Deck, DeckRegistry, Error, HostCallbacks, ORC_ABI_VERSION, ORC_TYPE_F32, ORC_TYPE_F64,
    ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
    ORC_TYPE_U64, OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcPlugin, OrcTypeId,
    OrcTypeInfo, ProxyType, TOrcData, TOrcPluginAdaptor, deck_from_proxy, orc_fn_info, orc_plugin,
    reset_handle,
};
use std::sync::{LazyLock, OnceLock};

#[global_allocator]
static ALLOCATOR: orc_sdk::PluginAllocator = orc_sdk::PluginAllocator::new();

static REGISTRY: LazyLock<DeckRegistry> = LazyLock::new(DeckRegistry::new);

static HOST: OnceLock<OrcHostCallbackAPI> = OnceLock::new();

pub(crate) fn host_callbacks() -> &'static OrcHostCallbackAPI {
    HOST.get().unwrap_or(&HostCallbacks::DUMMY_CALLBACKS)
}

pub(crate) fn registry() -> &'static DeckRegistry {
    &REGISTRY
}

struct Adaptor;

impl TOrcPluginAdaptor for Adaptor {
    fn host_callbacks() -> &'static OrcHostCallbackAPI {
        host_callbacks()
    }

    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin) -> Result<(), Error> {
        if host.abi_version != ORC_ABI_VERSION {
            return Err(Error::ABIVersionMismatch);
        }
        // Read host capabilities - Set up the allocator first, before any heap allocations happen.
        ALLOCATOR.init_from_host(host);
        match HOST.set(host.callbacks) {
            Ok(_) => {}
            Err(_) => return Err(Error::PluginAlreadyInitialized),
        }
        // Tell the host about the plugin provided types and functions
        out.abi_version = ORC_ABI_VERSION;
        out.name = c"math_plugin".as_ptr();
        out.desc = c"Math plugin to test and flesh out the SDK".as_ptr();
        out.n_types = ORC_EXPORTED_TYPES.len() as u64;
        out.types = ORC_EXPORTED_TYPES.as_ptr();
        out.n_functions = ORC_EXPORTED_FUNCTIONS.len() as u64;
        out.functions = ORC_EXPORTED_FUNCTIONS.as_ptr();
        Ok(())
    }

    fn deck_alloc(type_id: OrcTypeId, handle: &mut OrcHandle) -> Result<(), Error> {
        match type_id {
            ORC_TYPE_U8 => REGISTRY.alloc::<u8>(handle),
            ORC_TYPE_U16 => REGISTRY.alloc::<u16>(handle),
            ORC_TYPE_U32 => REGISTRY.alloc::<u32>(handle),
            ORC_TYPE_U64 => REGISTRY.alloc::<u64>(handle),
            ORC_TYPE_I8 => REGISTRY.alloc::<i8>(handle),
            ORC_TYPE_I16 => REGISTRY.alloc::<i16>(handle),
            ORC_TYPE_I32 => REGISTRY.alloc::<i32>(handle),
            ORC_TYPE_I64 => REGISTRY.alloc::<i64>(handle),
            ORC_TYPE_F32 => REGISTRY.alloc::<f32>(handle),
            ORC_TYPE_F64 => REGISTRY.alloc::<f64>(handle),
            complex::COMPLEX_NUM_TYPE_ID => REGISTRY.alloc::<complex::Complex>(handle),
            _ => Err(Error::DeckTypeMismatch),
        }
    }

    fn deck_free(handle: &mut OrcHandle) -> Result<(), Error> {
        match REGISTRY.free(handle.handle) {
            Ok(()) => {
                reset_handle(handle);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn deck_from_proxy(
        inputs: &[OrcHandle],
        proxy_type: ProxyType,
        proxy: &OrcHandle,
        out: &mut OrcHandle,
    ) -> Result<(), Error> {
        let type_id = match inputs.first() {
            Some(input) => input.type_id,
            None => return Err(Error::InvalidProxy),
        };
        match type_id {
            ORC_TYPE_U8 => deck_from_proxy::<u8>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_U16 => deck_from_proxy::<u16>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_U32 => deck_from_proxy::<u32>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_U64 => deck_from_proxy::<u64>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_I8 => deck_from_proxy::<i8>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_I16 => deck_from_proxy::<i16>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_I32 => deck_from_proxy::<i32>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_I64 => deck_from_proxy::<i64>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_F32 => deck_from_proxy::<f32>(inputs, proxy_type, proxy, out, &REGISTRY),
            ORC_TYPE_F64 => deck_from_proxy::<f64>(inputs, proxy_type, proxy, out, &REGISTRY),
            complex::COMPLEX_NUM_TYPE_ID => {
                deck_from_proxy::<complex::Complex>(inputs, proxy_type, proxy, out, &REGISTRY)
            }
            _ => Err(Error::DeckTypeMismatch),
        }
    }

    fn deck_serialize(
        _ctx: u64,
        handle: &OrcHandle,
        write: &mut impl std::io::Write,
    ) -> Result<(), Error> {
        match orc_sdk::try_serialize_handle(handle, write) {
            Err(Error::DeckTypeMismatch) => {}
            result => return result,
        }
        // Header already written by try_serialize_handle. Write custom item data.
        match handle.type_id {
            complex::COMPLEX_NUM_TYPE_ID => {
                let items = handle.items::<Complex>();
                let n_serialized =
                    Complex::serialize(items, write).map_err(|_| Error::SerializationError)?;
                if n_serialized != items.len() {
                    return Err(Error::SerializationError);
                }
                Ok(())
            }
            _ => Err(Error::DeckTypeMismatch),
        }
    }

    fn deck_deserialize(
        _ctx: u64,
        read: &mut impl std::io::Read,
        out: &mut OrcHandle,
    ) -> Result<(), Error> {
        let marks = match orc_sdk::try_deserialize_handle(read, out, &REGISTRY) {
            Ok(()) => return Ok(()),
            Err(marks) => marks,
        };
        // Header and marks already read. Read custom item data.
        match out.type_id {
            complex::COMPLEX_NUM_TYPE_ID => {
                let items = Complex::deserialize(read, out.n_items as usize)
                    .map_err(|_| Error::SerializationError)?;
                let mut deck = Deck::<Complex>::default();
                deck.assign_from_raw_data(items, marks);
                REGISTRY.alloc_with_value(Some(deck), out)?;
            }
            _ => return Err(Error::DeckTypeMismatch),
        }
        // Assert that all bytes are consumed for custom types.
        let mut trailing = [0u8; 1];
        if read.read(&mut trailing).unwrap_or(1) != 0 {
            return Err(Error::SerializationError);
        }
        Ok(())
    }
}

orc_plugin!(Adaptor);

mod basic;
mod complex;

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[
    orc_fn_info!(basic::add),
    orc_fn_info!(basic::multiply),
    orc_fn_info!(basic::subtract),
    orc_fn_info!(basic::divide),
    orc_fn_info!(basic::repeat_list),
    orc_fn_info!(basic::collatz_parallel_experiment),
    // Complex numbers.
    orc_fn_info!(complex::create_complex),
    orc_fn_info!(complex::add_complex),
    orc_fn_info!(complex::mul_complex),
    orc_fn_info!(complex::complex_get_parts),
];

const ORC_EXPORTED_TYPES: &[OrcTypeInfo] = &[Complex::TYPE_INFO];

#[cfg(test)]
mod tests {
    use super::*;
    use orc_sdk::{Deck, deck, update_handle_from_deck};
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(9000);

    fn next_id() -> u64 {
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    fn make_handle<T: TOrcData>(deck: &Deck<T>) -> OrcHandle {
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

    fn serialize_handle(handle: &OrcHandle) -> Vec<u8> {
        let mut buf = Vec::new();
        Adaptor::deck_serialize(0, handle, &mut buf).unwrap();
        buf
    }

    fn deserialize_handle(buf: &[u8], handle_id: u64) -> OrcHandle {
        let mut cursor = Cursor::new(buf);
        let mut out = OrcHandle {
            handle: handle_id,
            ..Default::default()
        };
        Adaptor::deck_deserialize(0, &mut cursor, &mut out).unwrap();
        out
    }

    // ==================== deck_serialize / deck_deserialize ====================

    #[test]
    fn t_serialize_round_trip_f64_flat() {
        let deck = deck![1.0_f64, 2.0, 3.0];
        let h = make_handle(&deck);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, ORC_TYPE_F64);
        assert_eq!(out.n_items, 3);
        assert_eq!(out.items::<f64>(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn t_serialize_round_trip_i32_flat() {
        let deck = deck![10_i32, 20, 30, 40];
        let h = make_handle(&deck);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, ORC_TYPE_I32);
        assert_eq!(out.n_items, 4);
        assert_eq!(out.items::<i32>(), &[10, 20, 30, 40]);
    }

    #[test]
    fn t_serialize_round_trip_u8_flat() {
        let deck = deck![255_u8, 0, 128];
        let h = make_handle(&deck);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, ORC_TYPE_U8);
        assert_eq!(out.items::<u8>(), &[255, 0, 128]);
    }

    #[test]
    fn t_serialize_round_trip_f64_nested() {
        let deck: Deck<f64> = deck![[1.0, 2.0], [3.0]];
        let h = make_handle(&deck);
        assert!(h.n_marks > 0);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, ORC_TYPE_F64);
        assert_eq!(out.n_items, 3);
        assert_eq!(out.items::<f64>(), &[1.0, 2.0, 3.0]);
        assert_eq!(out.n_marks, h.n_marks);
        let orig_marks = unsafe { std::slice::from_raw_parts(h.marks, h.n_marks as usize) };
        let out_marks = unsafe { std::slice::from_raw_parts(out.marks, out.n_marks as usize) };
        assert_eq!(orig_marks, out_marks);
    }

    #[test]
    fn t_serialize_round_trip_empty_deck() {
        let deck: Deck<f32> = Deck::default();
        let h = make_handle(&deck);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, ORC_TYPE_F32);
        assert_eq!(out.n_items, 0);
    }

    #[test]
    fn t_serialize_round_trip_complex() {
        let deck = deck![
            Complex::from_parts(1.0, 2.0),
            Complex::from_parts(-3.5, 0.0)
        ];
        let h = make_handle(&deck);
        assert_eq!(h.type_id, complex::COMPLEX_NUM_TYPE_ID);
        assert_eq!(h.n_items, 2);
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.type_id, complex::COMPLEX_NUM_TYPE_ID);
        assert_eq!(out.n_items, 2);
        assert_eq!(out.items::<Complex>(), deck.items());
    }

    #[test]
    fn t_serialize_preserves_dims() {
        let deck = deck![1.0_f64, 2.0];
        let mut h = make_handle(&deck);
        h.dims[0] = 1;
        h.dims[1] = -2;
        h.dims[3] = 3;
        let buf = serialize_handle(&h);
        let out = deserialize_handle(&buf, next_id());
        assert_eq!(out.dims, h.dims);
    }

    #[test]
    fn t_deserialize_trailing_bytes_fails() {
        let deck = deck![42_i64];
        let h = make_handle(&deck);
        let mut buf = serialize_handle(&h);
        buf.push(0xFF); // extra trailing byte
        let mut cursor = Cursor::new(&buf);
        let mut out = out_handle();
        let result = Adaptor::deck_deserialize(0, &mut cursor, &mut out);
        assert!(result.is_err());
    }

    #[test]
    fn t_deserialize_truncated_fails() {
        let deck = deck![1.0_f64, 2.0, 3.0];
        let h = make_handle(&deck);
        let buf = serialize_handle(&h);
        let truncated = &buf[..buf.len() - 1];
        let mut cursor = Cursor::new(truncated);
        let mut out = out_handle();
        let result = Adaptor::deck_deserialize(0, &mut cursor, &mut out);
        assert!(result.is_err());
    }

    #[test]
    fn t_deserialize_empty_buffer_fails() {
        let buf: &[u8] = &[];
        let mut cursor = Cursor::new(buf);
        let mut out = out_handle();
        let result = Adaptor::deck_deserialize(0, &mut cursor, &mut out);
        assert!(result.is_err());
    }

    // ==================== deck_alloc / deck_free ====================

    #[test]
    fn t_deck_alloc_f64() {
        let mut h = out_handle();
        Adaptor::deck_alloc(ORC_TYPE_F64, &mut h).unwrap();
        assert_eq!(h.type_id, ORC_TYPE_F64);
        assert_eq!(h.item_size, size_of::<f64>() as u64);
        assert_eq!(h.n_items, 0);
        assert!(h.free_fn.is_some());
        Adaptor::deck_free(&mut h).unwrap();
        assert!(h.items.is_null());
    }

    #[test]
    fn t_deck_alloc_complex() {
        let mut h = out_handle();
        Adaptor::deck_alloc(complex::COMPLEX_NUM_TYPE_ID, &mut h).unwrap();
        assert_eq!(h.type_id, complex::COMPLEX_NUM_TYPE_ID);
        assert_eq!(h.item_size, size_of::<Complex>() as u64);
        Adaptor::deck_free(&mut h).unwrap();
    }

    #[test]
    fn t_deck_alloc_unknown_type_fails() {
        let mut h = out_handle();
        let result = Adaptor::deck_alloc(0xDEAD, &mut h);
        assert!(result.is_err());
    }

    #[test]
    fn t_deck_alloc_preserves_handle_id() {
        let mut h = out_handle();
        let expected_id = h.handle;
        Adaptor::deck_alloc(ORC_TYPE_I32, &mut h).unwrap();
        assert_eq!(h.handle, expected_id);
        Adaptor::deck_free(&mut h).unwrap();
    }

    #[test]
    fn t_deck_free_sets_null() {
        let mut h = out_handle();
        Adaptor::deck_alloc(ORC_TYPE_F32, &mut h).unwrap();
        assert!(h.free_fn.is_some());
        Adaptor::deck_free(&mut h).unwrap();
        assert!(h.items.is_null());
        assert!(h.marks.is_null());
        assert_eq!(h.n_items, 0);
        assert_eq!(h.type_id, 0);
    }

    // ==================== all primitive types round-trip ====================

    #[test]
    fn t_serialize_round_trip_all_primitive_types() {
        // Test each primitive type serializes and deserializes correctly.
        macro_rules! test_type {
            ($ty:ty, [$($v:expr),+ $(,)?]) => {{
                let deck: Deck<$ty> = deck![$($v),+];
                let h = make_handle(&deck);
                let buf = serialize_handle(&h);
                let out = deserialize_handle(&buf, next_id());
                assert_eq!(out.type_id, <$ty as TOrcData>::TYPE_INFO.type_id);
                let expected: &[$ty] = &[$($v),+];
                assert_eq!(out.items::<$ty>(), expected);
            }};
        }
        test_type!(u8, [1, 2, 3]);
        test_type!(u16, [100, 200, 300]);
        test_type!(u32, [1000, 2000, 3000]);
        test_type!(u64, [10000, 20000, 30000]);
        test_type!(i8, [-1, 0, 1]);
        test_type!(i16, [-100, 0, 100]);
        test_type!(i32, [-1000, 0, 1000]);
        test_type!(i64, [-10000, 0, 10000]);
        test_type!(f32, [1.5, -2.5, 0.0]);
        test_type!(f64, [1.5, -2.5, 0.0]);
    }
}
