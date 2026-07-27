use orc_sdk::{
    Deck, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64, ORC_U8, ORC_U16, ORC_U32, ORC_U64,
    ObjectRegistry, OrcFuncInfo, OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId,
    ProxyType, TOrcData, TOrcPluginAdaptor, handle_from_deck, orc_fn, orc_fn_info, orc_plugin,
    reset_handle, slice_from_ptr, slice_from_ptr_mut,
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

#[orc_fn]
/// Adds the inputs together. This function supports all floating point and integer primitives.
unsafe extern "C" fn plugin_fn_add(
    ctx: u64,
    inputs: *const OrcHandle,
    n_inputs: u64,
    outputs: *mut OrcHandle,
    n_outputs: u64,
) {
    let (inputs, outputs) = unsafe {
        (
            slice_from_ptr(inputs, n_inputs as usize),
            slice_from_ptr_mut(outputs, n_outputs as usize),
        )
    };
    todo!(
        "
Implement a generic add function that supports any number of inputs, of any scalar or integer type,
as long as all the inputs and the one output handle are of the same type"
    );
}

#[orc_fn]
/// Multiplies the inputs together. This function supports all floating point and integer primitives.
unsafe extern "C" fn plugin_fn_mul(
    ctx: u64,
    inputs: *const OrcHandle,
    n_inputs: u64,
    outputs: *mut OrcHandle,
    n_outputs: u64,
) {
    todo!(
        "
Implement a generic multiply function that supports any number of inputs, of any scalar or integer type,
as long as all the inputs and the one output handle are of the same type"
    );
}

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] =
    &[orc_fn_info!(plugin_fn_add), orc_fn_info!(plugin_fn_mul)];
