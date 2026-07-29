use orc_sdk::{
    Combinations, Deck, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64, ORC_U8, ORC_U16,
    ORC_U32, ORC_U64, ObjectRegistry, OrcFuncInfo, OrcHandle, OrcHost, OrcItemProxy, OrcMark,
    OrcPlugin, OrcTypeId, ProxyType, TOrcData, TOrcPluginAdaptor, handle_from_deck, orc_fn_info,
    orc_generate_fn_info, orc_plugin, reset_handle, slice_from_ptr, slice_from_ptr_mut,
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
        _inputs: &[OrcHandle],
        _proxy_type: orc_sdk::ProxyType,
        _proxies: &[OrcItemProxy],
        _marks: &[OrcMark],
        _out: &mut OrcHandle,
    ) {
        todo!()
    }
}

orc_plugin!(Adaptor);

#[orc_generate_fn_info]
/// Adds the inputs together. This function supports all floating point and integer primitives.
unsafe extern "C" fn plugin_fn_add(
    _ctx: u64,
    inputs: *const OrcHandle,
    n_inputs: u64,
    outputs: *mut OrcHandle,
    n_outputs: u64,
) {
    assert!(n_outputs == 1, "This function only supports one output");
    assert!(n_inputs == 2, "This function only supports two inputs");
    let (inputs, outputs) = unsafe {
        (
            slice_from_ptr(inputs, n_inputs as usize),
            slice_from_ptr_mut(outputs, n_outputs as usize),
        )
    };
    // Output and input types must be the same, and must match one of the supported types.
    let type_id = outputs[0].type_id;
    if inputs[0].type_id != type_id || inputs[1].type_id != type_id {
        panic!("Type mismatch");
    }
    // List processing setup.
    let input_depths = vec![0u8; inputs.len()];
    const OUTPUT_DEPTHS: &[u8] = &[0];
    let mut comb = Combinations::from_handles(inputs, &input_depths, OUTPUT_DEPTHS)
        .expect("Cannot initialize combinations from the provide inputs");
    // TODO: I am hardcoding f64 for now, later I need to check the input types, and dispatch to different generic functions.
    let (input_slice_lhs, input_slice_rhs) = unsafe {
        (
            slice_from_ptr(inputs[0].items.cast::<f64>(), inputs[0].n_items as usize),
            slice_from_ptr(inputs[1].items.cast::<f64>(), inputs[1].n_items as usize),
        )
    };
    let result = REGISTRY.with_mut(
        &[outputs[0].handle],
        |out_decks| -> Result<(), orc_sdk::Error> {
            let out_deck: &mut Deck<f64> = out_decks[0]
                .downcast_mut()
                .ok_or(orc_sdk::Error::DeckTypeMismatch)?;
            // List processing iterations.
            loop {
                let mut out_view = comb.get_output(out_deck, 0);
                let output = out_view.push_default_mut();
                let lhs = comb.get_input(input_slice_lhs, 0);
                let rhs = comb.get_input(input_slice_rhs, 1);
                // The actual addition happens here.
                *output = lhs.as_ref() + rhs.as_ref();
                // Advance to the next list processing iteration.
                if !comb.advance() {
                    break;
                }
            }
            let out_id = outputs[0].handle;
            outputs[0] = handle_from_deck(out_deck, out_id);
            Ok(())
        },
    );
    if let Err(e) = result {
        panic!("Failed to run the function with the following error:\n{e:?}");
    }
}

#[orc_generate_fn_info]
/// Multiplies the inputs together. This function supports all floating point and integer primitives.
unsafe extern "C" fn plugin_fn_mul(
    _ctx: u64,
    _inputs: *const OrcHandle,
    _n_inputs: u64,
    _outputs: *mut OrcHandle,
    _n_outputs: u64,
) {
    todo!(
        "
Implement a generic multiply function that supports any number of inputs, of any scalar or integer type,
as long as all the inputs and the one output handle are of the same type"
    );
}

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] =
    &[orc_fn_info!(plugin_fn_add), orc_fn_info!(plugin_fn_mul)];
