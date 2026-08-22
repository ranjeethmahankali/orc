use libloading::Library;
use std::{collections::HashMap, collections::hash_map::Entry, path::Path};

use crate::{
    ContextArena, DeckAllocFn, DeckDeserializeFn, DeckFreeFn, DeckFromProxyFn, DeckSerializeFn,
    Error, FuncInfo, HostCallbacks, ORC_ABI_VERSION, ORC_DECK_PROXY_COPY_ALL,
    ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, OrcHandle, OrcHost, OrcPlugin, OrcTypeId,
    PRIMITIVE_TYPES, PluginInitFn, ProxyType, TypeInfo, ptr_from_slice, slice_from_ptr,
    util::string_from_ffi,
};

/// This is to store the info, handles etc. for a loaded plugin.
#[derive(Debug)]
pub struct Plugin {
    _lib: Library,
    name: String,
    desc: String,
    types: Box<[TypeInfo]>,
    functions: Box<[FuncInfo]>,
    deck_alloc: DeckAllocFn,
    deck_free: DeckFreeFn,
    deck_from_proxy: DeckFromProxyFn,
    deck_serialize: DeckSerializeFn,
    deck_deserialize: DeckDeserializeFn,
}

impl Plugin {
    const PLUGIN_INIT_FN_NAME: &str = "orc_plugin_init";
    const DECK_ALLOC_FN_NAME: &str = "orc_deck_alloc";
    const DECK_FREE_FN_NAME: &str = "orc_deck_free";
    const DECK_FROM_PROXY_FN_NAME: &str = "orc_deck_from_proxy";
    const DECK_SERIALIZE_FN_NAME: &str = "orc_deck_serialize";
    const DECK_DESERIALIZE_FN_NAME: &str = "orc_deck_deserialize";

    pub fn load(path: &Path, host: &OrcHost) -> Result<Self, String> {
        let lib = unsafe { Library::new(path) }.map_err(|e| format!("cannot load library: {e}"))?;
        let (init, deck_alloc, deck_free, deck_from_proxy, deck_serialize, deck_deserialize): (
            PluginInitFn,
            DeckAllocFn,
            DeckFreeFn,
            DeckFromProxyFn,
            DeckSerializeFn,
            DeckDeserializeFn,
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
                lib.get(Self::DECK_SERIALIZE_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::DECK_SERIALIZE_FN_NAME))?,
                lib.get(Self::DECK_DESERIALIZE_FN_NAME.as_bytes())
                    .map(|s| *s)
                    .map_err(|_| format!("missing symbol '{}'", Self::DECK_DESERIALIZE_FN_NAME))?,
            )
        };
        let mut plugin_data = OrcPlugin::default();
        let err = unsafe { init(host, &mut plugin_data) };
        match Error::from_raw(err) {
            Ok(()) => {} // Do nothing.
            Err(err) => return Err(format!("Unable to load the plugin: {err}")),
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
            deck_serialize,
            deck_deserialize,
        })
    }

    pub fn alloc_deck(&self, type_id: OrcTypeId, handle: &mut OrcHandle) -> Result<(), Error> {
        let err = unsafe { (self.deck_alloc)(type_id, handle) };
        Error::from_raw(err)
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
        out: &mut OrcHandle,
    ) -> Result<(), Error> {
        let ptype = match proxy_type {
            ProxyType::CopyAll => ORC_DECK_PROXY_COPY_ALL,
            ProxyType::CopyItems => ORC_DECK_PROXY_COPY_ITEMS,
            ProxyType::Shuffle => ORC_DECK_PROXY_SHUFFLE,
        };
        let err = unsafe {
            (self.deck_from_proxy)(inputs.as_ptr(), inputs.len() as u64, ptype, proxy, out)
        };
        Error::from_raw(err)
    }

    pub fn serialize_deck<R>(
        &self,
        arena: &ContextArena<Vec<u8>>,
        handle: &OrcHandle,
        vis: impl Fn(&mut Vec<u8>) -> R,
    ) -> Result<R, Error> {
        let ctx = arena.insert(|buf| buf.clear())?;
        let err = unsafe { (self.deck_serialize)(ctx, handle) };
        if let Err(e) = Error::from_raw(err) {
            // Consume the slot to avoid leaking it, but discard the data.
            let _ = arena.consume(ctx, |_| {});
            return Err(e);
        }
        arena.consume(ctx, vis)
    }

    pub fn deserialize_deck(&self, ctx: u64, buf: &[u8], out: &mut OrcHandle) -> Result<(), Error> {
        let err = unsafe {
            (self.deck_deserialize)(ctx, ptr_from_slice(buf).cast(), buf.len() as u64, out)
        };
        Error::from_raw(err)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn desc(&self) -> &str {
        &self.desc
    }
}

#[derive(Debug)]
pub enum TypeOwner {
    BuiltIn(TypeInfo),
    Plugin(usize, usize),
}

#[derive(Debug)]
pub struct PluginSet {
    plugins: Box<[Plugin]>,
    type_map: HashMap<OrcTypeId, TypeOwner>,
    function_map: HashMap<String, (usize, usize)>,
}

pub fn load_plugins(dir: &Path, host: &OrcHost) -> Result<PluginSet, Error> {
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
    let mut type_map = HashMap::<OrcTypeId, TypeOwner>::from_iter(
        PRIMITIVE_TYPES
            .iter()
            .map(|ti| (ti.type_id, TypeOwner::BuiltIn(TypeInfo::from(ti)))),
    );
    let mut function_map = HashMap::<String, (usize, usize)>::default();
    let mut plugins = Vec::<Plugin>::new();
    for entry in entries.into_iter().flatten() {
        let path = entry.path();
        match path.extension() {
            Some(ext) if ext == PLUGIN_EXT => {
                match Plugin::load(&path, host) {
                    Ok(plugin) => {
                        // Ensure the types in this new plugin don't conflict with the types already loaded.
                        for (type_index, type_info) in plugin.types().iter().enumerate() {
                            match type_map.entry(type_info.type_id) {
                                Entry::Occupied(occupied) => match occupied.get() {
                                    TypeOwner::BuiltIn(ti) => {
                                        callbacks.error(&format!(
                                            "Type {} of plugin {} conflicts with the builtin type {}.",
                                            type_info.name, plugin.name, ti.name
                                        ));
                                        return Err(Error::CannotLoadPlugins);
                                    }
                                    TypeOwner::Plugin(plugin_index, conflicting_type_index) => {
                                        callbacks.error(&format!(
                                        "The id of type {} from plugin {} conflicts with that of {} from {}.",
                                        plugins[*plugin_index].name,
                                        plugins[*plugin_index].types()[*conflicting_type_index].name,
                                        type_info.name,
                                        plugin.name));
                                        return Err(Error::CannotLoadPlugins);
                                    }
                                },
                                Entry::Vacant(vacant) => {
                                    vacant.insert(TypeOwner::Plugin(plugins.len(), type_index));
                                }
                            }
                        }
                        // Now ensure the functions in this plugin don't conflict with any functions already loaded.
                        for (fn_index, fn_info) in plugin.functions().iter().enumerate() {
                            match function_map.entry(fn_info.name.clone()) {
                                Entry::Occupied(occupied) => {
                                    let (plugin_index, _function_index) = occupied.get();
                                    callbacks.error(&format!(
                                        "Function {} in plugin {} conflicts with a function with the same name in plugin {}.",
                                        fn_info.name, plugin.name(), plugins[*plugin_index].name()));
                                    return Err(Error::CannotLoadPlugins);
                                }
                                Entry::Vacant(vacant) => {
                                    vacant.insert((plugins.len(), fn_index));
                                }
                            }
                        }
                        plugins.push(plugin);
                        callbacks.info(&format!("Loaded plugin: {}", path.display()));
                    }
                    Err(e) => {
                        callbacks.info(&format!("Skipping {}: {e}", path.display()));
                    }
                }
            }
            _ => {} // Not a shared library.
        }
    }
    Ok(PluginSet {
        plugins: plugins.into_boxed_slice(),
        type_map,
        function_map,
    })
}

impl PluginSet {
    pub fn get_function(&self, name: &str) -> Option<&FuncInfo> {
        self.function_map
            .get(name)
            .map(|(plugin_index, function_index)| {
                &self.plugins[*plugin_index].functions[*function_index]
            })
    }

    pub fn num_types(&self) -> usize {
        self.type_map.len()
    }

    pub fn num_plugins(&self) -> usize {
        self.plugins.len()
    }

    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn get_type_owner(&self, type_id: OrcTypeId) -> Option<&TypeOwner> {
        self.type_map.get(&type_id)
    }
}

#[cfg(test)]
pub(crate) mod test_harness {
    use crate::{ContextArena, DagError, DeckRegistry, OrcHost, PluginSet, Workflow, load_plugins};
    use std::sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    };

    pub fn plugin_dir() -> std::path::PathBuf {
        let exe = std::env::current_exe().unwrap();
        let deps = exe.parent().unwrap();
        if deps.ends_with("deps") {
            deps.parent().unwrap().to_path_buf()
        } else {
            deps.to_path_buf()
        }
    }

    // Load once per process — the plugin's OnceLock<HOST> can only be set once.
    pub static PLUGINS: LazyLock<PluginSet> =
        LazyLock::new(|| load_plugins(&plugin_dir(), &OrcHost::default()).unwrap());

    pub struct TestHarness {
        pub registry: DeckRegistry,
        pub arena: ContextArena<Vec<u8>>,
        pub handle_counter: AtomicU64,
    }

    impl TestHarness {
        pub fn new() -> Self {
            Self {
                registry: DeckRegistry::default(),
                arena: ContextArena::<Vec<u8>>::default(),
                handle_counter: AtomicU64::new(1),
            }
        }

        pub fn roundtrip(&self, wf: &Workflow) -> Workflow {
            let mut buf = Vec::new();
            wf.write_to_msgpack(&PLUGINS, &self.arena, &mut buf)
                .unwrap();
            let mut cursor = std::io::Cursor::new(&buf);
            let mut next_id = || self.handle_counter.fetch_add(1, Ordering::Relaxed);
            Workflow::read_from_msgpack(&mut cursor, &PLUGINS, &self.registry, 0, &mut next_id)
                .unwrap()
        }

        pub fn roundtrip_bytes(&self, wf: &Workflow) -> Vec<u8> {
            let mut buf = Vec::new();
            wf.write_to_msgpack(&PLUGINS, &self.arena, &mut buf)
                .unwrap();
            buf
        }

        pub fn read_workflow(&self, buf: &[u8]) -> Result<Workflow, DagError> {
            let mut cursor = std::io::Cursor::new(buf);
            let mut next_id = || self.handle_counter.fetch_add(1, Ordering::Relaxed);
            Workflow::read_from_msgpack(&mut cursor, &PLUGINS, &self.registry, 0, &mut next_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_harness::PLUGINS;
    use crate::{ORC_TYPE_F64, OrcHandle};

    #[test]
    fn alloc_deck_populates_handle() {
        let plugin = &PLUGINS.plugins()[0];
        let mut handle = OrcHandle {
            handle: 5000,
            ..Default::default()
        };
        let err = unsafe { (plugin.deck_alloc)(ORC_TYPE_F64, &mut handle) };
        assert_eq!(err, crate::ORC_ERROR_NONE);
        assert!(handle.free_fn.is_some());
        assert_eq!(handle.type_id, ORC_TYPE_F64);
        assert_eq!(handle.handle, 5000);
        handle.free();
    }

    #[test]
    fn free_deck_resets_handle() {
        let plugin = &PLUGINS.plugins()[0];
        let mut handle = OrcHandle {
            handle: 5001,
            ..Default::default()
        };
        unsafe { (plugin.deck_alloc)(ORC_TYPE_F64, &mut handle) };
        assert!(handle.free_fn.is_some());
        plugin.free_deck(&mut handle).unwrap();
        assert!(handle.free_fn.is_none());
        assert!(handle.items.is_null());
        assert_eq!(handle.handle, 5001);
    }
}
