use orc_sdk::{
    Deck, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64, ORC_U8, ORC_U16, ORC_U32, ORC_U64,
    ObjectRegistry, OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId, ProxyType,
    TOrcData, TOrcPluginAdaptor, handle_from_deck, orc_plugin, reset_handle,
};
use std::sync::LazyLock;

static REGISTRY: LazyLock<ObjectRegistry> = LazyLock::new(ObjectRegistry::new);

fn alloc_deck<T: TOrcData>() -> OrcHandle {
    let deck = Deck::<T>::default();
    let mut handle = handle_from_deck(&deck, 0);
    handle.handle = REGISTRY.alloc(deck).expect("Failed to alloc deck");
    handle
}

struct Adaptor;

impl TOrcPluginAdaptor for Adaptor {
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin) {
        todo!()
    }

    fn deck_alloc(id: OrcTypeId) -> OrcHandle {
        match id.primitive_id {
            ORC_U8 => alloc_deck::<u8>(),
            ORC_U16 => alloc_deck::<u16>(),
            ORC_U32 => alloc_deck::<u32>(),
            ORC_U64 => alloc_deck::<u64>(),
            ORC_I8 => alloc_deck::<i8>(),
            ORC_I16 => alloc_deck::<i16>(),
            ORC_I32 => alloc_deck::<i32>(),
            ORC_I64 => alloc_deck::<i64>(),
            ORC_F32 => alloc_deck::<f32>(),
            ORC_F64 => alloc_deck::<f64>(),
            _ => panic!("Unsupported type id: {}", id.primitive_id),
        }
    }

    fn deck_free(handle: &mut OrcHandle) {
        REGISTRY.free(handle.handle).expect("Failed to free deck");
        reset_handle(handle);
    }

    fn deck_from_proxy(
        inputs: &[OrcHandle],
        proxy_type: orc_sdk::ProxyType,
        proxies: &[OrcItemProxy],
        marks: &[OrcMark],
        out: &mut OrcHandle,
    ) {
        todo!()
    }
}

orc_plugin!(Adaptor);
