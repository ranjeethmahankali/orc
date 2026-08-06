use libloading::Library;
use std::{
    collections::{HashMap, hash_map::Entry},
    path::Path,
};

use crate::{
    BUILTIN_TYPES, DeckAllocFn, DeckFreeFn, DeckFromProxyFn, Error, FuncInfo, HostCallbacks,
    ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL, ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE,
    OrcHandle, OrcHost, OrcPlugin, OrcTypeId, PluginInitFn, ProxyType, TypeInfo, slice_from_ptr,
    util::string_from_ffi,
};

/// This is to store the info, handles etc. for a loaded plugin.
pub struct Plugin {
    _lib: Library,
    name: String,
    desc: String,
    types: Box<[TypeInfo]>,
    functions: Box<[FuncInfo]>,
    deck_alloc: DeckAllocFn,
    deck_free: DeckFreeFn,
    deck_from_proxy: DeckFromProxyFn,
}

impl Plugin {
    const PLUGIN_INIT_FN_NAME: &str = "orc_plugin_init";
    const DECK_ALLOC_FN_NAME: &str = "orc_deck_alloc";
    const DECK_FREE_FN_NAME: &str = "orc_deck_free";
    const DECK_FROM_PROXY_FN_NAME: &str = "orc_deck_from_proxy";

    pub fn load(path: &Path, host: &OrcHost) -> Result<Self, String> {
        let lib = unsafe { Library::new(path) }.map_err(|e| format!("cannot load library: {e}"))?;
        let (init, deck_alloc, deck_free, deck_from_proxy): (
            PluginInitFn,
            DeckAllocFn,
            DeckFreeFn,
            DeckFromProxyFn,
        ) = unsafe {
            (
                lib.get(Self::PLUGIN_INIT_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::PLUGIN_INIT_FN_NAME))?,
                lib.get(Self::DECK_ALLOC_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::DECK_ALLOC_FN_NAME))?,
                lib.get(Self::DECK_FREE_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::DECK_FREE_FN_NAME))?,
                lib.get(Self::DECK_FROM_PROXY_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::DECK_FROM_PROXY_FN_NAME))?,
            )
        };
        let mut plugin_data = OrcPlugin::default();
        let err = unsafe { init(host, &mut plugin_data) };
        match err {
            crate::ORC_ERROR_NONE => {} // Do nothing.
            crate::ORC_ERROR_ABI_VERSION_MISMATCH => {
                return Err(
                    "Unable to load the plugin because of ABI version mismatch.".to_string()
                );
            }
            _ => {
                return Err(format!(
                    "Unable to load the plugin. Error code: {err:#010x}"
                ));
            }
        }
        if plugin_data.abi_version != ORC_ABI_VERSION {
            return Err("Unable to load the plugin because of ABI version mismatch.".to_string());
        }
        Ok(Plugin {
            _lib: lib,
            name: string_from_ffi(plugin_data.name.cast()),
            desc: string_from_ffi(plugin_data.desc.cast()),
            types: unsafe {
                slice_from_ptr(plugin_data.types, plugin_data.n_types as usize)
                    .iter()
                    .map(TypeInfo::from)
                    .collect()
            },
            functions: unsafe {
                slice_from_ptr(plugin_data.functions, plugin_data.n_functions as usize)
                    .iter()
                    .map(FuncInfo::from)
                    .collect()
            },
            deck_alloc,
            deck_free,
            deck_from_proxy,
        })
    }

    pub fn alloc_deck(&self, type_id: OrcTypeId) -> Result<OrcHandle, Error> {
        let mut handle = OrcHandle::default();
        let err = unsafe { (self.deck_alloc)(type_id, &mut handle) };
        Error::from_raw(err).map(|_| handle)
    }

    pub fn free_deck(&self, handle: &mut OrcHandle) -> Result<(), Error> {
        let err = unsafe { (self.deck_free)(handle) };
        Error::from_raw(err)
    }

    pub fn types(&self) -> &[TypeInfo] {
        &self.types
    }

    pub fn functions(&self) -> &[FuncInfo] {
        &self.functions
    }

    pub fn create_proxy_deck(
        &self,
        inputs: &[OrcHandle],
        proxy_type: ProxyType,
        proxy: &OrcHandle,
    ) -> Result<OrcHandle, Error> {
        let mut out = OrcHandle::default();
        let ptype = match proxy_type {
            ProxyType::CopyAll => ORC_DECK_PROXY_COPY_ALL,
            ProxyType::CopyItems => ORC_DECK_PROXY_COPY_ITEMS,
            ProxyType::Shuffle => ORC_DECK_PROXY_SHUFFLE,
        };
        let err = unsafe {
            (self.deck_from_proxy)(inputs.as_ptr(), inputs.len() as u64, ptype, proxy, &mut out)
        };
        Error::from_raw(err).map(|_| out)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn desc(&self) -> &str {
        &self.desc
    }
}

pub fn load_plugins(dir: &Path, host: &OrcHost) -> Result<Box<[Plugin]>, Error> {
    let callbacks = HostCallbacks {
        inner: host.callbacks,
        context: 0,
    };
    let entries = std::fs::read_dir(dir).map_err(|_e| Error::CannotLoadPlugins)?;
    #[cfg(target_os = "windows")]
    const PLUGIN_EXT: &str = "dll";
    #[cfg(target_os = "macos")]
    const PLUGIN_EXT: &str = "dylib";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    const PLUGIN_EXT: &str = "so";
    // We're going to load the plugins one at a time, and accumulate the type_ids in this hashmap.
    let mut type_map =
        HashMap::<OrcTypeId, (String, String)>::from_iter(BUILTIN_TYPES.iter().map(|ti| {
            (
                ti.type_id,
                ("built-in".to_string(), string_from_ffi(ti.name.cast())),
            )
        }));
    let mut plugins = Vec::<Plugin>::new();
    for entry in entries.into_iter().flatten() {
        let path = entry.path();
        match path.extension() {
            Some(ext) if ext == PLUGIN_EXT => match Plugin::load(&path, host) {
                Ok(plugin) => {
                    // Ensure the types in this new plugin don't conflict with the types already loaded.
                    for type_info in plugin.types() {
                        match type_map.entry(type_info.type_id) {
                            Entry::Occupied(occupied) => {
                                let (plugin_name, type_name) = occupied.get();
                                callbacks.error(&format!(
                                    "The id of type {} from plugin {} conflicts with that of {} from {}.",
                                    type_name,
                                    plugin_name,
                                    type_info.name,
                                    plugin.name));
                                return Err(Error::CannotLoadPlugins);
                            }
                            Entry::Vacant(vacant) => {
                                vacant.insert((plugin.name.clone(), type_info.name.clone()));
                            }
                        }
                    }
                    callbacks.info(&format!("Loaded plugin: {}", path.display()));
                    plugins.push(plugin);
                }
                Err(e) => {
                    callbacks.info(&format!("Skipping {}: {e}", path.display()));
                }
            },
            _ => {} // Not a shared library.
        }
    }
    Ok(plugins.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ORC_TYPE_F64, OrcHandle};
    use std::sync::LazyLock;

    fn math_plugin_path() -> std::path::PathBuf {
        // The test binary is in build/debug/deps/; built DLLs are in build/debug/.
        let exe = std::env::current_exe().unwrap();
        let deps = exe.parent().unwrap();
        let dir = if deps.ends_with("deps") {
            deps.parent().unwrap()
        } else {
            deps
        };
        #[cfg(target_os = "windows")]
        return dir.join("math_plugin.dll");
        #[cfg(target_os = "macos")]
        return dir.join("libmath_plugin.dylib");
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return dir.join("libmath_plugin.so");
    }

    // Load once per process — the plugin's OnceLock<HOST> can only be set once.
    static PLUGIN: LazyLock<Plugin> =
        LazyLock::new(|| Plugin::load(&math_plugin_path(), &OrcHost::default()).unwrap());

    #[test]
    fn alloc_deck_populates_handle() {
        let mut handle = OrcHandle {
            handle: 5000,
            ..Default::default()
        };
        let err = unsafe { (PLUGIN.deck_alloc)(ORC_TYPE_F64, &mut handle) };
        assert_eq!(err, crate::ORC_ERROR_NONE);
        assert!(handle.free_fn.is_some());
        assert_eq!(handle.type_id, ORC_TYPE_F64);
        assert_eq!(handle.handle, 5000);
        handle.free();
    }

    #[test]
    fn free_deck_resets_handle() {
        let mut handle = OrcHandle {
            handle: 5001,
            ..Default::default()
        };
        unsafe { (PLUGIN.deck_alloc)(ORC_TYPE_F64, &mut handle) };
        assert!(handle.free_fn.is_some());
        PLUGIN.free_deck(&mut handle).unwrap();
        assert!(handle.free_fn.is_none());
        assert!(handle.items.is_null());
        assert_eq!(handle.handle, 5001);
    }
}
