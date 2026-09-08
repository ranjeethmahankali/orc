mod app;
mod canvas;
mod context_menu;
mod exec;
mod inspect;
mod interaction;
mod layout;
mod render;
mod state;

use eframe::egui;
use orc_sdk::{
    ContextArena, DeckRegistry, Error, ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL,
    ORC_ERROR_INVALID_HANDLE, ORC_ERROR_INVALID_PROXY, ORC_ERROR_NONE, ORC_TYPE_F32, ORC_TYPE_F64,
    ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32,
    ORC_TYPE_U64, OrcError, OrcHandle, OrcHandleBorrowed, OrcHost, OrcHostCallbackAPI,
    OrcHostMemoryAPI, OrcProxyType, PluginSet, ProxyType, TypeOwner, Workflow, reset_handle,
    slice_from_ptr,
};
use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{CStr, c_void};
use std::sync::{
    LazyLock,
    atomic::{AtomicU64, Ordering},
};

pub(crate) static REGISTRY: LazyLock<DeckRegistry> = LazyLock::new(DeckRegistry::new);
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
        return ORC_ERROR_INVALID_HANDLE;
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
        None => return ORC_ERROR_INVALID_PROXY,
    };
    if inputs.iter().skip(1).any(|h| h.type_id != type_id) {
        return ORC_ERROR_INVALID_PROXY;
    }
    let plugin_set: &PluginSet = &PLUGIN_SET;
    let proxy_type = match proxy_type {
        ORC_DECK_PROXY_COPY_ALL => ProxyType::CopyAll,
        orc_sdk::ORC_DECK_PROXY_COPY_ITEMS => ProxyType::CopyItems,
        orc_sdk::ORC_DECK_PROXY_SHUFFLE => ProxyType::Shuffle,
        _ => return ORC_ERROR_INVALID_PROXY,
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
    eprintln!(
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
        exe_dir.parent().unwrap()
    } else {
        exe_dir
    };
    PluginSet::load_from_dir(plugin_dir, &HOST).expect("Failed to load plugins")
});

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

fn load_workflow(path: &str) -> Workflow {
    let file = std::fs::File::open(path).expect("Failed to open workflow file");
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    Workflow::read_from_msgpack(&mut reader, &PLUGIN_SET, &REGISTRY, 0, &mut next_id)
        .expect("Failed to deserialize workflow")
}

fn main() -> eframe::Result {
    // Force plugin loading at startup.
    let plugin_set: &PluginSet = &PLUGIN_SET;
    eprintln!("Loaded {} plugin(s)", plugin_set.num_plugins());
    for plugin in plugin_set.plugins() {
        eprintln!(
            "  {} ({} functions)",
            plugin.name(),
            plugin.functions().len()
        );
    }

    let workflow = match std::env::args().nth(1) {
        Some(path) => {
            eprintln!("Loading workflow from: {path}");
            load_workflow(&path)
        }
        None => {
            eprintln!("No workflow file specified, starting with empty workflow");
            Workflow::default()
        }
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Dagger",
        options,
        Box::new(|cc| Ok(Box::new(app::DaggerApp::new(cc, workflow)))),
    )
}

#[cfg(test)]
mod test {
    use super::PLUGIN_SET;
    use orc_sdk::{FuncInfo, PluginSet};

    /// Plugins are loaded from the directory holding the binary, which on a tree where only
    /// cargo has run may not contain them yet. Skip loudly rather than fail.
    fn lookup(name: &str) -> Option<FuncInfo> {
        let plugin_set: &PluginSet = &PLUGIN_SET;
        let found = plugin_set.get_function(name).cloned();
        if found.is_none() {
            println!("skipping: no plugin providing {name} was loaded");
        }
        found
    }

    /// Pin labels come from the argument names the plugin declares over the ABI, so a
    /// regression in that FFI conversion would silently blank every label in the editor.
    #[test]
    fn t_declared_argument_names_survive_the_abi() {
        let Some(add) = lookup("add") else { return };
        assert_eq!(add.n_inputs, Some(2));
        assert_eq!(add.n_outputs, Some(1));
        let inputs: Vec<&str> = add.input_args.iter().map(|a| a.name.as_str()).collect();
        let outputs: Vec<&str> = add.output_args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(inputs, ["lhs", "rhs"]);
        assert_eq!(outputs, ["out"]);
    }

    /// A function may declare a concrete arity while leaving its argument arrays null, meaning
    /// any type and no names. Those pins have to stay bare instead of picking up junk.
    #[test]
    fn t_null_argument_arrays_yield_no_names() {
        let Some(func) = lookup("list_length") else {
            return;
        };
        assert_eq!(func.n_inputs, Some(1));
        assert!(func.input_args.is_empty());
        assert!(func.output_args.is_empty());
    }

    /// Variadic functions declare no arity, so there is nothing to read the arrays against.
    #[test]
    fn t_variadic_functions_have_no_declared_args() {
        let Some(func) = lookup("flatten_deck") else {
            return;
        };
        assert_eq!(func.n_inputs, None);
        assert_eq!(func.n_outputs, None);
        assert!(func.input_args.is_empty());
        assert!(func.output_args.is_empty());
    }
}
