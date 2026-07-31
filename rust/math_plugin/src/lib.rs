use orc_sdk::{
    Combinations, Deck, Error, HostCallbacks, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64,
    ORC_U8, ORC_U16, ORC_U32, ORC_U64, ObjectRegistry, OrcDims, OrcFuncInfo, OrcHandle, OrcHost,
    OrcHostCallbackAPI, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId, ProxyType, TOrcData,
    TOrcPluginAdaptor, handle_from_deck, orc_assert_return, orc_fn, orc_fn_info,
    orc_generate_fn_info, orc_plugin, reset_handle, slice_from_ptr, slice_from_ptr_mut,
};
use std::{
    ops::{Div, Mul},
    sync::{LazyLock, OnceLock},
};

#[global_allocator]
static ALLOCATOR: orc_sdk::PluginAllocator = orc_sdk::PluginAllocator::new();

static REGISTRY: LazyLock<ObjectRegistry> = LazyLock::new(ObjectRegistry::new);

static HOST: OnceLock<OrcHostCallbackAPI> = OnceLock::new();

fn host_callbacks() -> &'static OrcHostCallbackAPI {
    HOST.get().unwrap_or(&HostCallbacks::DUMMY_CALLBACKS)
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
        match HOST.set(host.callbacks) {
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

orc_fn!(plugin_fn_add, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = &REGISTRY;

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Multiplies two inputs values. This function supports any integer or floating point scalar
    /// types. The two inputs must be of the same type. The output produced will be of the same type
    /// also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Mul<Output = T> + Copy,
    {
        *out = *lhs * *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(plugin_fn_mul, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = &REGISTRY;

    type Types = (
        Case<f32>,
        Case<f64>,
        Case<u8>,
        Case<u16>,
        Case<u32>,
        Case<u64>,
        Case<i8>,
        Case<i16>,
        Case<i32>,
        Case<i64>,
    );

    /// Adds two inputs values. This function supports any integer or floating point scalar
    /// types. The two inputs must be of the same type. The output produced will be of the same type
    /// also.
    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Mul<Output = T> + Copy,
    {
        *out = *lhs * *rhs;
        Ok(())
    }

    /// The dimensions of both inputs must be the same. The output dimensions will match that.
    fn dims(lhs: &OrcDims, rhs: &OrcDims, out: &mut OrcDims) -> Result<(), Error> {
        if rhs != lhs {
            return Err(Error::InvalidDimensions);
        }
        *out = *lhs;
        Ok(())
    }
});

orc_fn!(plugin_fn_div, {
    let host_callbacks: &HostCallbacks = host_callbacks();
    let registry: &ObjectRegistry = &REGISTRY;

    type Types = (Case<f32>, Case<f64>);

    fn run<T>(_host: &HostCallbacks, lhs: &T, rhs: &T, out: &mut T) -> Result<(), Error>
    where
        T: TOrcData + Div<Output = T> + Copy,
    {
        *out = *lhs / *rhs;
        Ok(())
    }
});

const ORC_EXPORTED_FUNCTIONS: &[OrcFuncInfo] =
    &[orc_fn_info!(plugin_fn_add), orc_fn_info!(plugin_fn_mul)];
