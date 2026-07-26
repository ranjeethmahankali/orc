use orc_sdk::{
    Deck, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64, ORC_U8, ORC_U16, ORC_U32, ORC_U64,
    ObjectRegistry, OrcFuncInfo, OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId,
    ProxyType, TOrcData, TOrcPluginAdaptor, handle_from_deck, orc_plugin, reset_handle,
};
use std::sync::LazyLock;

#[global_allocator]
static ALLOCATOR: orc_sdk::PluginAllocator = orc_sdk::PluginAllocator::new();

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
        // Read host capabilities - Set up the allocator first, before any heap allocations happen.
        ALLOCATOR.init_from_host(host);
        // Tell the host about the plugin provided types and functions.
        out.n_types = 0;
        out.types = std::ptr::null();
        out.n_functions = ORC_EXPORTED_FUNCTIONS.len() as u64;
        out.functions = ORC_EXPORTED_FUNCTIONS.as_ptr();
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

// TODO: I am hard coding this right now, but we should probably think about using proc macros to
// automatically generate this metadata for the functions. I am imagining something very ergonomic,
// that lets me write a simple Rust docstring, and turns that into the metadata for the function.

const ORC_FN_ADD_INFO: OrcFuncInfo = OrcFuncInfo {
    name: c"add".as_ptr(),
    desc: c"Adds the inputs togehter. This function supports all floating point and integer primitives.".as_ptr(),
    func: Some(plugin_fn_add),
};

unsafe extern "C" fn plugin_fn_add(
    ctx: u64,
    inputs: *const OrcHandle,
    n_inputs: usize,
    outputs: *mut OrcHandle,
    n_outputs: usize,
) {
    todo!(
        "
Implement a generic add function that supports any number of inputs, of any scalar or integer type,
as long as all the inputs and the one output handle are of the same type"
    );
}

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] = &[ORC_FN_ADD_INFO];
