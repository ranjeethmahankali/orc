use arc::Arc2d;
use orc_sdk::{
    Deck, DeckRegistry, Error, HostCallbacks, ORC_ABI_VERSION, ORC_TYPE_F32, ORC_TYPE_F64,
    ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
    ORC_TYPE_U64, OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcPlugin, OrcTypeId,
    OrcTypeInfo, ProxyType, TOrcData, TOrcPluginAdaptor, deck_from_proxy, orc_fn_info, orc_plugin,
    reset_handle, to_str_deck,
};
use std::sync::{LazyLock, OnceLock};

mod arc;

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

struct PluginAdaptor;

impl TOrcPluginAdaptor for PluginAdaptor {
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
        out.name = c"example_curve_plugin".as_ptr();
        out.desc = c"Example plugin with some curve functions.".as_ptr();
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
            arc::ARC_TYPE_ID => REGISTRY.alloc::<Arc2d>(handle),
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
            arc::ARC_TYPE_ID => deck_from_proxy::<Arc2d>(inputs, proxy_type, proxy, out, &REGISTRY),
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
            arc::ARC_TYPE_ID => {
                let items = handle.items::<Arc2d>();
                let n_serialized =
                    Arc2d::serialize(items, write).map_err(|_| Error::SerializationError)?;
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
            arc::ARC_TYPE_ID => {
                let items = Arc2d::deserialize(read, out.n_items as usize)
                    .map_err(|_| Error::SerializationError)?;
                let mut deck = Deck::<Arc2d>::default();
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

    fn deck_to_str(input: &OrcHandle, out: &mut OrcHandle) -> Result<(), Error> {
        REGISTRY.alloc::<u8>(out)?;
        let type_id = input.type_id;
        REGISTRY
            .with_mut(&[out.handle], |decks| -> Result<(), Error> {
                let deck = decks[0]
                    .downcast_mut::<Deck<u8>>()
                    .ok_or(Error::DeckTypeMismatch)?;
                let result = match type_id {
                    ORC_TYPE_U8 => to_str_deck::<u8>(input, deck),
                    ORC_TYPE_U16 => to_str_deck::<u16>(input, deck),
                    ORC_TYPE_U32 => to_str_deck::<u32>(input, deck),
                    ORC_TYPE_U64 => to_str_deck::<u64>(input, deck),
                    ORC_TYPE_I8 => to_str_deck::<i8>(input, deck),
                    ORC_TYPE_I16 => to_str_deck::<i16>(input, deck),
                    ORC_TYPE_I32 => to_str_deck::<i32>(input, deck),
                    ORC_TYPE_I64 => to_str_deck::<i64>(input, deck),
                    ORC_TYPE_F32 => to_str_deck::<f32>(input, deck),
                    ORC_TYPE_F64 => to_str_deck::<f64>(input, deck),
                    arc::ARC_TYPE_ID => to_str_deck::<Arc2d>(input, deck),
                    _ => Err(Error::DeckTypeMismatch),
                };
                if result.is_ok() {
                    unsafe { orc_sdk::update_handle_from_deck(deck, out) };
                }
                result
            })
            .flatten()
    }
}

orc_plugin!(PluginAdaptor);

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[];

const ORC_EXPORTED_TYPES: &[OrcTypeInfo] = &[Arc2d::TYPE_INFO];
