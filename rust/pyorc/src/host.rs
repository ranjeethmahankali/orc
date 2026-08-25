use orc_sdk::{
    ContextArena, DeckRegistry, Error, ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL,
    ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, ORC_ERROR_NONE, ORC_TYPE_F32, ORC_TYPE_F64,
    ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
    ORC_TYPE_U64, OrcError, OrcHandle, OrcHandleBorrowed, OrcHost, OrcHostCallbackAPI,
    OrcHostMemoryAPI, OrcProxyType, PluginSet, ProxyType, TypeOwner, reset_handle, slice_from_ptr,
};
use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{CStr, c_void};
use std::sync::{LazyLock, Mutex, atomic::AtomicU64};

pub static PLUGIN_SET: LazyLock<Mutex<PluginSet>> =
    LazyLock::new(|| Mutex::new(PluginSet::default()));
pub static REGISTRY: LazyLock<DeckRegistry> = LazyLock::new(DeckRegistry::new);
pub static HANDLE_COUNTER: AtomicU64 = AtomicU64::new(0);
pub static SERIAL_CONTEXT_ARENA: LazyLock<ContextArena<Vec<u8>>> =
    LazyLock::new(ContextArena::default);

unsafe extern "C" fn host_alloc(size: u64, alignment: u64) -> *mut c_void {
    Layout::from_size_align(size as usize, alignment as usize)
        .map(|layout| unsafe { alloc(layout) as *mut c_void })
        .unwrap_or(std::ptr::null_mut())
}

unsafe extern "C" fn host_dealloc(ptr: *mut c_void, size: u64, alignment: u64) {
    // Ignoring the error. Nothing much we can do.
    let _ = Layout::from_size_align(size as usize, alignment as usize)
        .map(|layout| unsafe { dealloc(ptr as *mut u8, layout) });
}

unsafe extern "C" fn serial_write_callback(ctx: u64, data: *const c_void, len: u64) -> OrcError {
    let incoming_slice: &[u8] = unsafe { slice_from_ptr(data.cast(), len as usize) };
    match SERIAL_CONTEXT_ARENA.visit_mut(ctx, |buf| buf.extend_from_slice(incoming_slice)) {
        Ok(_) => ORC_ERROR_NONE,
        Err(e) => e.into(),
    }
}

unsafe extern "C" fn host_create_proxy_deck(
    inputs: *const OrcHandle,
    n_inputs: u64,
    proxy_type: OrcProxyType,
    proxy: *const OrcHandle,
    out: *mut OrcHandle,
) -> OrcError {
    if inputs.is_null() || proxy.is_null() || out.is_null() {
        return orc_sdk::ORC_ERROR_INVALID_HANDLE;
    }
    let (inputs, proxy, out) = unsafe {
        (
            slice_from_ptr(inputs, n_inputs as usize),
            &*proxy,
            &mut *out,
        )
    };
    let type_id = match inputs.first() {
        Some(input) => input.type_id,
        None => return orc_sdk::ORC_ERROR_INVALID_PROXY,
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        return orc_sdk::ORC_ERROR_INVALID_PROXY;
    }
    let plugin_set = match PLUGIN_SET.lock() {
        Ok(ps) => ps,
        Err(_) => return orc_sdk::ORC_ERROR_CONCURRENCY_PROBLEM,
    };
    let proxy_type = match proxy_type {
        ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
        ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
        ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
        _ => return orc_sdk::ORC_ERROR_INVALID_PROXY,
    };
    let result = match plugin_set.get_type_owner(type_id) {
        Some(type_owner) => match type_owner {
            TypeOwner::BuiltIn(_) => match type_id {
                ORC_TYPE_U8 => {
                    orc_sdk::deck_from_proxy::<u8>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_U16 => {
                    orc_sdk::deck_from_proxy::<u16>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_U32 => {
                    orc_sdk::deck_from_proxy::<u32>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_U64 => {
                    orc_sdk::deck_from_proxy::<u64>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_I8 => {
                    orc_sdk::deck_from_proxy::<i8>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_I16 => {
                    orc_sdk::deck_from_proxy::<i16>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_I32 => {
                    orc_sdk::deck_from_proxy::<i32>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_I64 => {
                    orc_sdk::deck_from_proxy::<i64>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_F32 => {
                    orc_sdk::deck_from_proxy::<f32>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                ORC_TYPE_F64 => {
                    orc_sdk::deck_from_proxy::<f64>(inputs, proxy_type, proxy, out, &REGISTRY)
                }
                _ => return orc_sdk::ORC_ERROR_INVALID_PROXY,
            },
            TypeOwner::Plugin(plugin_index, _) => {
                let plugin = &plugin_set.plugins()[*plugin_index];
                plugin.create_proxy_deck(inputs, proxy_type, proxy, out)
            }
        },
        None => return orc_sdk::ORC_ERROR_INVALID_PROXY,
    };
    if let Err(e) = result {
        return e.into();
    }
    ORC_ERROR_NONE
}

unsafe extern "C" fn report_message(
    ctx: u64,
    level: orc_sdk::OrcMessageLevel,
    msg: *const std::ffi::c_char,
) {
    let msg = if msg.is_null() {
        ""
    } else {
        &unsafe { CStr::from_ptr(msg) }.to_string_lossy()
    };
    println!(
        "[{}][{}] {}",
        match level {
            orc_sdk::ORC_MSG_LEVEL_DEBUG => "DEBUG",
            orc_sdk::ORC_MSG_LEVEL_INFO => "INFO",
            orc_sdk::ORC_MSG_LEVEL_WARN => "WARN",
            orc_sdk::ORC_MSG_LEVEL_ERROR => "ERROR",
            orc_sdk::ORC_MSG_LEVEL_FATAL => "FATAL",
            _ => "FATAL",
        },
        ctx,
        msg
    );
}

pub const HOST: OrcHost = OrcHost {
    abi_version: ORC_ABI_VERSION,
    memory_api: OrcHostMemoryAPI {
        alloc: Some(host_alloc),
        dealloc: Some(host_dealloc),
    },
    callbacks: OrcHostCallbackAPI {
        report_progress: None,
        report_message: Some(report_message),
        check_cancellation: None,
        report_intermediate_output: None,
        serial_write: Some(serial_write_callback),
    },
    create_deck_from_proxy: Some(host_create_proxy_deck),
};

/// # SAFETY
///
/// C ABI function used by the SDK to free host-allocated decks in REGISTRY.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orc_deck_free(handle: *mut OrcHandle) -> OrcError {
    if handle.is_null() {
        return ORC_ERROR_NONE;
    }
    let handle = unsafe { &mut *handle };
    match REGISTRY.free(handle.handle) {
        Ok(()) => {
            reset_handle(handle);
            ORC_ERROR_NONE
        }
        Err(e) => e.into(),
    }
}

/// Clone an `OrcHandle` by creating a full copy of its backing data via the proxy mechanism.
///
/// `ORC_DECK_PROXY_COPY_ALL` performs a deep copy and ignores the `proxy` parameter entirely,
/// so an empty default handle is passed as a harmless placeholder.
pub fn host_clone_orc_handle(src: OrcHandleBorrowed) -> Result<OrcHandle, Error> {
    let mut out = OrcHandle::default();
    let err = unsafe {
        host_create_proxy_deck(
            src.inner(),
            1,
            ORC_DECK_PROXY_COPY_ALL,
            &OrcHandle::default(), // ignored by CopyAll
            &mut out,
        )
    };
    Error::from_raw(err).map(|()| out)
}
