use orc_sdk::{
    DeckRegistry, Error, HostCallbacks, ORC_ABI_VERSION, ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8,
    ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
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

    fn deck_serialize(handle: &OrcHandle, write: impl std::io::Write) -> std::io::Result<()> {
        todo!()
    }

    fn deck_deserialize(read: impl std::io::Read, out: &mut OrcHandle) -> std::io::Result<()> {
        todo!()
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

const ORC_EXPORTED_TYPES: &[OrcTypeInfo] = &[complex::Complex::TYPE_INFO];
