use orc_sdk::{
    Deck, DeckRegistry, DeckView, Error, HostCallbacks, ORC_ABI_VERSION, ORC_TYPE_F32,
    ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16,
    ORC_TYPE_U32, ORC_TYPE_U64, OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcItemProxy,
    OrcPlugin, OrcTypeId, ProxyType, TOrcData, TOrcPluginAdaptor, orc_fn_info, orc_plugin,
    reset_handle, update_handle_from_deck,
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

fn alloc_deck<T: TOrcData>(handle: &mut OrcHandle) -> Result<(), Error> {
    REGISTRY.alloc::<T>(handle)
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
        out.n_types = 0;
        out.types = std::ptr::null();
        out.n_functions = ORC_EXPORTED_FUNCTIONS.len() as u64;
        out.functions = ORC_EXPORTED_FUNCTIONS.as_ptr();
        Ok(())
    }

    fn deck_alloc(type_id: OrcTypeId, handle: &mut OrcHandle) -> Result<(), Error> {
        match type_id {
            ORC_TYPE_U8 => alloc_deck::<u8>(handle),
            ORC_TYPE_U16 => alloc_deck::<u16>(handle),
            ORC_TYPE_U32 => alloc_deck::<u32>(handle),
            ORC_TYPE_U64 => alloc_deck::<u64>(handle),
            ORC_TYPE_I8 => alloc_deck::<i8>(handle),
            ORC_TYPE_I16 => alloc_deck::<i16>(handle),
            ORC_TYPE_I32 => alloc_deck::<i32>(handle),
            ORC_TYPE_I64 => alloc_deck::<i64>(handle),
            ORC_TYPE_F32 => alloc_deck::<f32>(handle),
            ORC_TYPE_F64 => alloc_deck::<f64>(handle),
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
            ORC_TYPE_U8 => deck_from_proxy_impl::<u8>(inputs, proxy_type, proxy, out),
            ORC_TYPE_U16 => deck_from_proxy_impl::<u16>(inputs, proxy_type, proxy, out),
            ORC_TYPE_U32 => deck_from_proxy_impl::<u32>(inputs, proxy_type, proxy, out),
            ORC_TYPE_U64 => deck_from_proxy_impl::<u64>(inputs, proxy_type, proxy, out),
            ORC_TYPE_I8 => deck_from_proxy_impl::<i8>(inputs, proxy_type, proxy, out),
            ORC_TYPE_I16 => deck_from_proxy_impl::<i16>(inputs, proxy_type, proxy, out),
            ORC_TYPE_I32 => deck_from_proxy_impl::<i32>(inputs, proxy_type, proxy, out),
            ORC_TYPE_I64 => deck_from_proxy_impl::<i64>(inputs, proxy_type, proxy, out),
            ORC_TYPE_F32 => deck_from_proxy_impl::<f32>(inputs, proxy_type, proxy, out),
            ORC_TYPE_F64 => deck_from_proxy_impl::<f64>(inputs, proxy_type, proxy, out),
            _ => Err(Error::DeckTypeMismatch),
        }
    }
}

fn deck_from_proxy_impl<T: TOrcData>(
    inputs: &[OrcHandle],
    proxy_type: ProxyType,
    proxy: &OrcHandle,
    out: &mut OrcHandle,
) -> Result<(), Error> {
    let type_id = match inputs.first() {
        Some(input) => input.type_id,
        None => return Err(Error::InvalidProxy),
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        // All inputs must be of the same type. This is a problem.
        return Err(Error::InvalidProxy);
    }
    out.dims = proxy.dims;
    REGISTRY.alloc::<T>(out)?;
    REGISTRY
        .with_mut(&[out.handle], |out_decks| -> Result<(), Error> {
            let out_deck = out_decks[0]
                .downcast_mut::<Deck<T>>()
                .ok_or(Error::DeckTypeMismatch)?;
            let (items, marks) = match proxy_type {
                ProxyType::CopyAll => {
                    // We expect exactly one input, and we will make a full clone of that data.
                    if inputs.len() != 1 {
                        return Err(Error::InvalidProxy);
                    }
                    let input_handle = unsafe { inputs.get_unchecked(0) }; // SAFETY: we just checked above.
                    let input = DeckView::<T>::from_handle(input_handle)?;
                    (input.items().to_vec(), input.marks().to_vec())
                }
                ProxyType::CopyItems => {
                    // We expect exactly one input. We will copy the items of the input, but the marks from the proxy.
                    if inputs.len() != 1 {
                        return Err(Error::InvalidProxy);
                    }
                    let input_handle = unsafe { inputs.get_unchecked(0) }; // SAFETY: we just checked above.
                    let input = DeckView::<T>::from_handle(input_handle)?;
                    let proxy = DeckView::<OrcItemProxy>::from_handle(proxy)?;
                    (input.items().to_vec(), proxy.marks().to_vec())
                }
                ProxyType::Shuffle => {
                    let proxy = DeckView::<OrcItemProxy>::from_handle(proxy)?;
                    let inputs = inputs
                        .iter()
                        .map(|input| DeckView::<T>::from_handle(input))
                        .collect::<Result<Box<[DeckView<T>]>, Error>>()?;
                    (
                        proxy
                            .items()
                            .iter()
                            .map(|ii| inputs[ii.tree as usize].items()[ii.item as usize].clone())
                            .collect::<Vec<T>>(),
                        proxy.marks().to_vec(),
                    )
                }
            };
            out_deck.assign_from_raw_data(items, marks);
            unsafe { update_handle_from_deck(out_deck, out) }; // SAFETY: we pulled the deck out of the same handle.
            Ok(())
        })
        .flatten()
}

orc_plugin!(Adaptor);

mod basic;

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[
    orc_fn_info!(basic::add),
    orc_fn_info!(basic::mul),
    orc_fn_info!(basic::sub),
    orc_fn_info!(basic::div),
    orc_fn_info!(basic::repeat_list),
];
