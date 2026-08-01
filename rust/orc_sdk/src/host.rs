use libloading::Library;
use std::{
    collections::{HashMap, hash_map::Entry},
    ffi::CStr,
    path::Path,
};

use crate::{
    DeckAllocFn, DeckFreeFn, DeckFromProxyFn, Error, FuncInfo, HostCallbacks, ORC_ABI_VERSION,
    ORC_DECK_PROXY_COPY_ALL, ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, OrcHandle, OrcHost,
    OrcPlugin, OrcTypeId, PluginInitFn, ProxyType, TypeInfo, slice_from_ptr,
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
            _ => return Err(format!("Unable to load the plugin. Error code: {err}")),
        }
        if plugin_data.abi_version != ORC_ABI_VERSION {
            return Err("Unable to load the plugin because of ABI version mismatch.".to_string());
        }
        Ok(Plugin {
            _lib: lib,
            name: unsafe { CStr::from_ptr(plugin_data.name) }
                .to_string_lossy()
                .into_owned(),
            desc: unsafe { CStr::from_ptr(plugin_data.desc) }
                .to_string_lossy()
                .into_owned(),
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
    let mut type_map = HashMap::<OrcTypeId, (usize, usize)>::new();
    let mut plugins = Vec::<Plugin>::new();
    for entry in entries.into_iter().flatten() {
        let path = entry.path();
        match path.extension() {
            Some(ext) if ext == PLUGIN_EXT => match Plugin::load(&path, host) {
                Ok(plugin) => {
                    let current_plugin_index = plugins.len();
                    // Ensure the types in this new plugin don't conflict with the types already loaded.
                    for (ti, type_info) in plugin.types().iter().enumerate() {
                        match type_map.entry(type_info.type_id) {
                            Entry::Occupied(occupied) => {
                                let (plugin_index, type_index) = *occupied.get();
                                callbacks.error(&format!(
                                    "The id of type {} from plugin {} conflicts with that of {} from {}.",
                                    plugins[plugin_index].types()[type_index].name,
                                    plugins[plugin_index].name,
                                    type_info.name,
                                    plugin.name));
                                return Err(Error::CannotLoadPlugins);
                            }
                            Entry::Vacant(vacant) => {
                                vacant.insert((current_plugin_index, ti));
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
