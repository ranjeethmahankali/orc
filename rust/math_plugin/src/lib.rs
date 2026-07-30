use orc_sdk::{
    Combinations, Deck, Error, HostCallbacks, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64,
    ORC_U8, ORC_U16, ORC_U32, ORC_U64, ObjectRegistry, OrcFuncInfo, OrcHandle, OrcHost,
    OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId, ProxyType, TOrcData, TOrcPluginAdaptor,
    handle_from_deck, orc_assert_return, orc_fn, orc_fn_info, orc_generate_fn_info, orc_plugin,
    reset_handle, slice_from_ptr, slice_from_ptr_mut,
};
use std::{
    ops::Mul,
    sync::{LazyLock, OnceLock},
};

#[global_allocator]
static ALLOCATOR: orc_sdk::PluginAllocator = orc_sdk::PluginAllocator::new();

static REGISTRY: LazyLock<ObjectRegistry> = LazyLock::new(ObjectRegistry::new);

static HOST: OnceLock<HostCallbacks> = OnceLock::new();

fn host() -> &'static HostCallbacks {
    HOST.get().unwrap_or(&HostCallbacks::DUMMY)
}

fn alloc_deck<T: TOrcData>() -> Result<OrcHandle, Error> {
    let deck = Deck::<T>::default();
    let mut handle = handle_from_deck(&deck, 0);
    match REGISTRY.alloc(deck) {
        Ok(h) => handle.handle = h,
        Err(e) => {
            // We're not reporting anything to the host, other than zeroing out this handle. For now
            // that is enough to communicate to the caller that the allocation didn't happen.
            reset_handle(&mut handle);
            return Err(e);
        }
    };
    Ok(handle)
}

struct Adaptor;

impl TOrcPluginAdaptor for Adaptor {
    fn plugin_init(host: &OrcHost, out: &mut OrcPlugin) -> Result<(), Error> {
        // Read host capabilities - Set up the allocator first, before any heap allocations happen.
        ALLOCATOR.init_from_host(host);
        match HOST.set(HostCallbacks {
            inner: host.callbacks,
        }) {
            Ok(_) => {}
            Err(_) => return Err(Error::PluginInitError),
        }
        // Tell the host about the plugin provided types and functions.
        out.n_types = 0;
        out.types = std::ptr::null();
        out.n_functions = ORC_EXPORTED_FUNCTIONS.len() as u64;
        out.functions = ORC_EXPORTED_FUNCTIONS.as_ptr();
        Ok(())
    }

    fn deck_alloc(id: OrcTypeId) -> Result<OrcHandle, Error> {
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
            // We just return an empty handle to the host when the type is not supported. Maybe in
            // the future we should return some error code.
            _ => Err(Error::DeckTypeMismatch),
        }
    }

    fn deck_free(handle: &mut OrcHandle) -> Result<(), Error> {
        // We're intentionally ignoring the error for now. Maybe in the future we update the ABI to
        // allow reporting an error.
        REGISTRY.free(handle.handle)
    }

    fn deck_from_proxy(
        _inputs: &[OrcHandle],
        _proxy_type: orc_sdk::ProxyType,
        _proxies: &[OrcItemProxy],
        _marks: &[OrcMark],
        _out: &mut OrcHandle,
    ) -> Result<(), Error> {
        todo!()
    }
}

orc_plugin!(Adaptor);

#[orc_generate_fn_info]
/// Adds the inputs together. This function supports all floating point and integer primitives.
unsafe extern "C" fn plugin_fn_add(
    ctx: u64,
    inputs: *const OrcHandle,
    n_inputs: u64,
    outputs: *mut OrcHandle,
    n_outputs: u64,
) {
    orc_assert_return!(
        host(),
        ctx,
        n_inputs == 2,
        "This function only supports two inputs"
    );
    orc_assert_return!(
        host(),
        ctx,
        n_outputs == 1,
        "This function only supports one output"
    );
    let (inputs, outputs) = unsafe {
        (
            slice_from_ptr(inputs, n_inputs as usize),
            slice_from_ptr_mut(outputs, n_outputs as usize),
        )
    };
    // Inputs must be of the same type.
    // Output and input types must be the same, and must match one of the supported types.
    let type_id = inputs[0].type_id;
    orc_assert_return!(
        host(),
        ctx,
        type_id == inputs[1].type_id,
        "Two inputs must be of the same type"
    );
    // List processing setup.
    let input_depths = vec![0u8; inputs.len()];
    const OUTPUT_DEPTHS: &[u8] = &[0];
    let mut comb = match Combinations::from_handles(inputs, &input_depths, OUTPUT_DEPTHS) {
        Ok(comb) => comb,
        Err(e) => {
            host().error(
                ctx,
                &format!("Cannot initialize combinations from the provided inputs. Error: {e:?}"),
            );
            return;
        }
    };
    let registry = &REGISTRY;
    // TODO: I am hardcoding f64 for now, later I need to check the input types, and dispatch to different generic functions.
    for output in outputs.iter_mut() {
        let alloc_result = registry.ensure_alloc_default::<Deck<f64>>(&mut output.handle);
        orc_assert_return!(
            host(),
            ctx,
            alloc_result.is_ok(),
            "Unable to allocate output deck"
        );
    }
    let (input_slice_lhs, input_slice_rhs) = unsafe {
        (
            slice_from_ptr(inputs[0].items.cast::<f64>(), inputs[0].n_items as usize),
            slice_from_ptr(inputs[1].items.cast::<f64>(), inputs[1].n_items as usize),
        )
    };
    let result = registry
        .with_mut(
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
        )
        .flatten();
    if let Err(e) = result {
        host().error(
            ctx,
            &format!("Failed to run the function with the following error:\n{e:?}"),
        );
    }
}

orc_fn! { plugin_fn_mul {
    /// Multiplies two inputs values. This function supports any integer or floating point scalar
    /// types. The two inputs must be of the same type. The output produced will be of the same type
    /// also.

    let host: &HostCallbacks = host();
    let registry: &ObjectRegistry = &REGISTRY;

    type Types = (Case<f32>, Case<f64>, Case<u8>, Case<u16>, Case<u32>,
                  Case<u64>, Case<i8>, Case<i16>, Case<i32>, Case<i64>);

    fn run<T: TOrcData + Mul<Output=T> + Copy>(_ctx: u64, lhs: &T, rhs: &T, out: &mut T) {
        *out = *lhs * *rhs;
    }
}}

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] =
    &[orc_fn_info!(plugin_fn_add), orc_fn_info!(plugin_fn_mul)];
