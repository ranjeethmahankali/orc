use libloading::Library;
use orc_sdk::{
    Deck, ORC_F64, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, OrcItemProxy,
    OrcPlugin, OrcTypeId, PluginInfo, deck, handle_from_deck,
};
use std::{ffi::CStr, path::Path};

type PluginInitFn = unsafe extern "C" fn(*const OrcHost, *mut OrcPlugin);
type DeckAllocFn = unsafe extern "C" fn(OrcTypeId, *mut OrcHandle);
type DeckFreeFn = unsafe extern "C" fn(*mut OrcHandle);
type DeckFromProxyFn =
    unsafe extern "C" fn(*const OrcHandle, u64, u32, *const OrcItemProxy, *mut OrcHandle);

struct Plugin {
    _lib: Library,
    deck_alloc: DeckAllocFn,
    deck_free: DeckFreeFn,
    deck_from_proxy: DeckFromProxyFn,
    info: PluginInfo,
}
const PLUGIN_INIT_FN_NAME: &str = "orc_plugin_init";
const DECK_ALLOC_FN_NAME: &str = "orc_deck_alloc";
const DECK_FREE_FN_NAME: &str = "orc_deck_free";
const DECK_FROM_PROXY_FN_NAME: &str = "orc_deck_from_proxy";

unsafe extern "C" fn report_message(ctx: u64, level: u32, msg: *const ::std::os::raw::c_char) {
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

impl Plugin {
    fn load(path: &Path) -> Result<Self, String> {
        let lib = unsafe { Library::new(path) }.map_err(|e| format!("cannot load library: {e}"))?;
        let (init, deck_alloc, deck_free, deck_from_proxy): (
            PluginInitFn,
            DeckAllocFn,
            DeckFreeFn,
            DeckFromProxyFn,
        ) = unsafe {
            (
                lib.get(PLUGIN_INIT_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{PLUGIN_INIT_FN_NAME}'"))?,
                lib.get(DECK_ALLOC_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{DECK_ALLOC_FN_NAME}'"))?,
                lib.get(DECK_FREE_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{DECK_FREE_FN_NAME}'"))?,
                lib.get(DECK_FROM_PROXY_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{DECK_FROM_PROXY_FN_NAME}'"))?,
            )
        };
        let host = OrcHost {
            memory_api: OrcHostMemoryAPI {
                alloc: None,
                dealloc: None,
            },
            callbacks: OrcHostCallbackAPI {
                report_progress: None,
                report_message: Some(report_message),
                check_cancellation: None,
                report_intermediate_output: None,
            },
        };
        let mut plugin_data = unsafe { std::mem::zeroed::<OrcPlugin>() };
        unsafe { init(&host, &mut plugin_data) };
        let info = PluginInfo::from(&plugin_data);
        Ok(Plugin {
            _lib: lib,
            deck_alloc,
            deck_free,
            deck_from_proxy,
            info,
        })
    }

    fn alloc_deck(&self, type_id: OrcTypeId) -> OrcHandle {
        let mut handle = unsafe { std::mem::zeroed::<OrcHandle>() };
        unsafe { (self.deck_alloc)(type_id, &mut handle) };
        handle
    }

    fn free_deck(&self, handle: &mut OrcHandle) {
        unsafe { (self.deck_free)(handle) };
    }
}

fn load_plugins(dir: &Path) -> Box<[Plugin]> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Cannot read plugin directory {}: {e}", dir.display());
            return Default::default();
        }
    };
    #[cfg(target_os = "windows")]
    const PLUGIN_EXT: &str = "dll";
    #[cfg(target_os = "macos")]
    const PLUGIN_EXT: &str = "dylib";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    const PLUGIN_EXT: &str = "so";

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            match path.extension() {
                Some(ext) if ext == PLUGIN_EXT => match Plugin::load(&path) {
                    Ok(plugin) => Some(plugin),
                    Err(e) => {
                        eprintln!("  Skipping {}: {e}", path.display());
                        None
                    }
                },
                _ => None,
            }
        })
        .collect()
}

fn main() {
    let exe_dir = std::env::current_exe()
        .expect("Cannot determine executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf();
    println!("Loading plugins from {}", exe_dir.display());
    let plugins = load_plugins(&exe_dir);
    println!("Loaded {} plugin(s)\n", plugins.len());
    // Print the loaded plugins and functions.
    for plugin in &plugins {
        println!("{} function(s):", plugin.info.functions.len());
        for func in plugin.info.functions.iter() {
            println!("  - {}\n    {}", func.name, func.desc);
        }
    }
    // Test the add function.
    let math_plugin = &plugins[0];
    let add_fn = math_plugin.info.functions[0].func;
    let a: Deck<f64> = deck![1.0, 2.0, 3.0];
    let b: Deck<f64> = deck![10.0, 20.0, 30.0];
    let mut out_handle = math_plugin.alloc_deck(OrcTypeId {
        primitive_id: ORC_F64,
        opaque_id: 0,
    });
    let inputs: &[OrcHandle] = &[handle_from_deck(&a, 0), handle_from_deck(&b, 1)];
    unsafe {
        add_fn(0, inputs.as_ptr(), inputs.len() as u64, &mut out_handle, 1);
    }
    // Print the output data.
    println!("Output deck: \n{}", out_handle.display::<f64>());
}
