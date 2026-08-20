use complex::Complex;
use orc_sdk::{
    Deck, DeckRegistry, Error, HostCallbacks, ORC_ABI_VERSION, ORC_TYPE_F32, ORC_TYPE_F64,
    ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
    ORC_TYPE_U64, OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcMark, OrcPlugin,
    OrcTypeId, OrcTypeInfo, ProxyType, TOrcData, TOrcPluginAdaptor, deck_from_proxy, orc_fn_info,
    orc_plugin, reset_handle,
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

    fn deck_serialize(handle: &OrcHandle, write: &mut impl std::io::Write) -> Result<(), Error> {
        fn as_bytes<T>(slice: &[T]) -> &[u8] {
            unsafe { std::slice::from_raw_parts(slice.as_ptr().cast(), size_of_val(slice)) }
        }

        orc_sdk::write_orc_handle_header(handle, write).map_err(|_| Error::SerializationError)?;
        let result = match handle.type_id {
            ORC_TYPE_U8 => write.write_all(as_bytes(handle.items::<u8>())),
            ORC_TYPE_U16 => write.write_all(as_bytes(handle.items::<u16>())),
            ORC_TYPE_U32 => write.write_all(as_bytes(handle.items::<u32>())),
            ORC_TYPE_U64 => write.write_all(as_bytes(handle.items::<u64>())),
            ORC_TYPE_I8 => write.write_all(as_bytes(handle.items::<i8>())),
            ORC_TYPE_I16 => write.write_all(as_bytes(handle.items::<i16>())),
            ORC_TYPE_I32 => write.write_all(as_bytes(handle.items::<i32>())),
            ORC_TYPE_I64 => write.write_all(as_bytes(handle.items::<i64>())),
            ORC_TYPE_F32 => write.write_all(as_bytes(handle.items::<f32>())),
            ORC_TYPE_F64 => write.write_all(as_bytes(handle.items::<f64>())),
            complex::COMPLEX_NUM_TYPE_ID => {
                let items = handle.items::<Complex>();
                let n_serialized =
                    Complex::serialize(items, write).map_err(|_| Error::SerializationError)?;
                if n_serialized != items.len() {
                    return Err(Error::SerializationError);
                }
                Ok(())
            }
            _ => return Err(Error::DeckTypeMismatch),
        };
        result.map_err(|_| Error::SerializationError)
    }

    fn deck_deserialize(read: &mut impl std::io::Read, out: &mut OrcHandle) -> Result<(), Error> {
        fn read_primitive_items<T: TOrcData + Default + Sized + Copy>(
            read: &mut impl std::io::Read,
            marks: Vec<OrcMark>,
            n_items: usize,
            handle: &mut OrcHandle,
        ) -> Result<(), Error> {
            let mut items = vec![T::default(); n_items];
            let bytes = unsafe {
                orc_sdk::slice_from_ptr_mut(
                    items.as_mut_ptr().cast::<u8>(),
                    n_items * size_of::<T>(),
                )
            };
            read.read_exact(bytes)
                .map_err(|_| Error::SerializationError)?;
            let items = items;
            let mut deck = Deck::<T>::default();
            deck.assign_from_raw_data(items, marks);
            REGISTRY.alloc_with_value(deck, handle)
        }
        let marks =
            orc_sdk::read_orc_handle_header(out, read).map_err(|_| Error::SerializationError)?;
        match out.type_id {
            ORC_TYPE_U8 => read_primitive_items::<u8>(read, marks, out.n_items as usize, out),
            ORC_TYPE_U16 => read_primitive_items::<u16>(read, marks, out.n_items as usize, out),
            ORC_TYPE_U32 => read_primitive_items::<u32>(read, marks, out.n_items as usize, out),
            ORC_TYPE_U64 => read_primitive_items::<u64>(read, marks, out.n_items as usize, out),
            ORC_TYPE_I8 => read_primitive_items::<i8>(read, marks, out.n_items as usize, out),
            ORC_TYPE_I16 => read_primitive_items::<i16>(read, marks, out.n_items as usize, out),
            ORC_TYPE_I32 => read_primitive_items::<i32>(read, marks, out.n_items as usize, out),
            ORC_TYPE_I64 => read_primitive_items::<i64>(read, marks, out.n_items as usize, out),
            ORC_TYPE_F32 => read_primitive_items::<f32>(read, marks, out.n_items as usize, out),
            ORC_TYPE_F64 => read_primitive_items::<f64>(read, marks, out.n_items as usize, out),
            complex::COMPLEX_NUM_TYPE_ID => {
                let items = Complex::deserialize(read, out.n_items as usize)
                    .map_err(|_| Error::SerializationError)?;
                let mut deck = Deck::<Complex>::default();
                deck.assign_from_raw_data(items, marks);
                REGISTRY.alloc_with_value(deck, out)
            }
            _ => return Err(Error::DeckTypeMismatch),
        }
    }
}

orc_plugin!(Adaptor);

mod basic;
mod complex;

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[
    orc_fn_info!(basic::add),
    orc_fn_info!(basic::mul),
    orc_fn_info!(basic::sub),
    orc_fn_info!(basic::div),
    orc_fn_info!(basic::repeat_list),
    orc_fn_info!(basic::collatz_parallel_experiment),
    // Complex numbers.
    orc_fn_info!(complex::create_complex),
    orc_fn_info!(complex::add_complex),
    orc_fn_info!(complex::mul_complex),
    orc_fn_info!(complex::complex_get_parts),
];

const ORC_EXPORTED_TYPES: &[OrcTypeInfo] = &[Complex::TYPE_INFO];
