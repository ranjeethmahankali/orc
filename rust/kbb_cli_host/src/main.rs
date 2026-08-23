#[cfg(test)]
mod test;

use orc_sdk::{
    ContextArena, DeckRegistry, Error, ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL,
    ORC_ERROR_INVALID_PROXY, ORC_ERROR_NONE, ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16,
    ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64, OrcError,
    OrcHandle, OrcHandleBorrowed, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, OrcProxyType,
    PluginSet, ProxyType, TypeOwner, orc_inline_dag, reset_handle, slice_from_ptr,
};
use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{CStr, c_void};
use std::sync::{LazyLock, atomic::AtomicU64};

// The host can allocate it's own decks, this registry is for that.
static REGISTRY: LazyLock<DeckRegistry> = LazyLock::new(DeckRegistry::new);
pub static HANDLE_COUNTER: AtomicU64 = AtomicU64::new(0);
static SERIAL_CONTEXT_ARENA: LazyLock<ContextArena<Vec<u8>>> = LazyLock::new(ContextArena::default);

unsafe extern "C" fn host_alloc(size: u64, alignment: u64) -> *mut c_void {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { alloc(layout) as *mut c_void }
}

unsafe extern "C" fn host_dealloc(ptr: *mut c_void, size: u64, alignment: u64) {
    let layout = Layout::from_size_align(size as usize, alignment as usize).unwrap();
    unsafe { dealloc(ptr as *mut u8, layout) }
}

unsafe extern "C" fn serial_write_callback(
    ctx: u64,
    data: *const std::ffi::c_void,
    len: u64,
) -> OrcError {
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
    // Convert all the FFI pointers to Rust references.
    let (inputs, proxy, out) = unsafe {
        (
            orc_sdk::slice_from_ptr(inputs, n_inputs as usize),
            &*proxy,
            &mut *out,
        )
    };
    let type_id = match inputs.first() {
        Some(input) => input.type_id,
        None => return ORC_ERROR_INVALID_PROXY,
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        // All inputs must be of the same type. This is a problem.
        return ORC_ERROR_INVALID_PROXY;
    }
    let plugin_set: &PluginSet = &PLUGIN_SET;
    // If the type is a primitive type, the host will allocate the deck on it's own.
    let proxy_type = match proxy_type {
        orc_sdk::ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
        orc_sdk::ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
        orc_sdk::ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
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
                _ => return ORC_ERROR_INVALID_PROXY,
            },
            TypeOwner::Plugin(plugin_index, _) => {
                let plugin = &plugin_set.plugins()[*plugin_index];
                plugin.create_proxy_deck(inputs, proxy_type, proxy, out)
            }
        },
        None => return ORC_ERROR_INVALID_PROXY,
    };
    if let Err(e) = result {
        return e.into();
    }
    ORC_ERROR_NONE
}

/// # SAFETY
///
/// This is a C ABI compatible function. Meant to be used by the SDK machiner.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orc_deck_free(handle: *mut orc_sdk::OrcHandle) -> orc_sdk::OrcError {
    if handle.is_null() {
        return orc_sdk::ORC_ERROR_NONE; // Nothing to free.
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

pub static PLUGIN_SET: LazyLock<PluginSet> = LazyLock::new(|| {
    let exe = std::env::current_exe().expect("Cannot determine executable path");
    let exe_dir = exe.parent().expect("Executable has no parent directory");
    let plugin_dir = if exe_dir.ends_with("deps") {
        // This is necessary to find the plugins from a test binary.
        exe_dir.parent().unwrap()
    } else {
        exe_dir
    };
    orc_sdk::load_plugins(plugin_dir, &HOST).expect("Failed to load plugins")
});

/// Helper function to requrest the global host to clone an OrcHandle, by deferring the cloning logic to the appropriate plugin.
pub fn host_clone_orc_handle(src: OrcHandleBorrowed) -> Result<OrcHandle, Error> {
    let mut out = OrcHandle::default();
    let err = unsafe {
        host_create_proxy_deck(
            src.inner(),
            1,
            ORC_DECK_PROXY_COPY_ALL,
            &OrcHandle::default(),
            &mut out,
        )
    };
    Error::from_raw(err).map(|()| out)
}

fn main() -> Result<(), Error> {
    let plugin_set: &PluginSet = &PLUGIN_SET;
    println!("Loaded {} plugin(s)\n", plugin_set.num_plugins());
    // Print the loaded plugins and functions.
    for plugin in plugin_set.plugins() {
        println!(
            "{} plugin has {} function(s):",
            plugin.name(),
            plugin.functions().len()
        );
        for func in plugin.functions().iter() {
            println!("  - {}\n    {}", func.name, func.desc);
        }
        println!();
    }
    let a_plus_b = orc_inline_dag!(plugin_set, &HANDLE_COUNTER, &*REGISTRY, {
        (add
         (const [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
         (const [10.0, 20.0, 30.0]))
    })?;
    println!(
        "math_plugin add([1,2,3], [10,20,30]):\n{}",
        a_plus_b.display::<f64>()
    );
    let len_a = orc_inline_dag!(plugin_set, &HANDLE_COUNTER, &*REGISTRY, {
        (list_length (const [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]]))
    })?;
    println!("List length output:\n{}", len_a.display::<u64>());
    let flat_c = orc_inline_dag!(plugin_set, &HANDLE_COUNTER, &*REGISTRY, {
        (flatten_deck (const [[1.0, 2.0, 3.0], [4.0, 5.0]]))
    })?;
    println!(
        "flatten_deck([[1,2,3],[4,5]]):\n{}",
        flat_c.display::<f64>()
    );
    let fmad_abc = orc_inline_dag!(plugin_set, &HANDLE_COUNTER, &*REGISTRY, {
        (let m (mul
                (const [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
                (const [10.0, 20.0, 30.0])))
            (add m (const [[1.0, 2.0, 3.0], [4.0, 5.0]]))
    })?;
    println!("mul_add_a_b_c:\n{}", fmad_abc.display::<f64>());
    let complex_result = orc_inline_dag!(plugin_set, &HANDLE_COUNTER, &*REGISTRY, {
        (let a (const [[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]]))
        (let b (const [10.0, 20.0, 30.0]))
        (let c (create_complex a b))
        (let c2 (create_complex b a))
        (let (real imag) (complex_get_parts (flatten_deck (mul_complex c c2))))
        (add real imag)
    })?;
    println!("complex_result:\n{}", complex_result.display::<f64>());
    Ok(())
}
