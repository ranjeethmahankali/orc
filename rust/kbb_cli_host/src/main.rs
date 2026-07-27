use libloading::Library;
use orc_sdk::{
    OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, OrcItemProxy, OrcPlugin,
    OrcTypeId,
};
use std::ffi::CStr;
use std::path::Path;

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
    plugin_data: OrcPlugin,
}
const PLUGIN_INIT_FN_NAME: &str = "orc_plugin_init";
const DECK_ALLOC_FN_NAME: &str = "orc_deck_alloc";
const DECK_FREE_FN_NAME: &str = "orc_deck_free";
const DECK_FROM_PROXY_FN_NAME: &str = "orc_deck_from_proxy";

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
        // let deck_free: DeckFreeFn = unsafe { get_sym(&lib, b)? };
        // let deck_from_proxy: DeckFromProxyFn = unsafe { get_sym(&lib, b"orc_deck_from_proxy")? };
        let host = OrcHost {
            memory_api: OrcHostMemoryAPI {
                alloc: None,
                dealloc: None,
            },
            callbacks: OrcHostCallbackAPI {
                report_progress: None,
                report_error: None,
                report_warning: None,
                check_cancellation: None,
            },
        };
        let mut plugin_data = unsafe { std::mem::zeroed::<OrcPlugin>() };
        unsafe { init(&host, &mut plugin_data) };
        Ok(Plugin {
            _lib: lib,
            deck_alloc,
            deck_free,
            deck_from_proxy,
            plugin_data,
        })
    }

    fn functions(&self) -> &[OrcFuncInfo] {
        if self.plugin_data.n_functions == 0 || self.plugin_data.functions.is_null() {
            return &[];
        }
        unsafe {
            std::slice::from_raw_parts(
                self.plugin_data.functions,
                self.plugin_data.n_functions as usize,
            )
        }
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
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            match path.extension() {
                Some(ext) if ext == "so" => match Plugin::load(&path) {
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

    for plugin in &plugins {
        let functions = plugin.functions();
        println!("{} function(s):", functions.len());
        for func in functions {
            let (name, desc) = unsafe { (CStr::from_ptr(func.name), CStr::from_ptr(func.desc)) };
            println!(
                "  - {}\n    {}",
                name.to_string_lossy(),
                desc.to_string_lossy()
            );
        }
    }
}
