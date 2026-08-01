use orc_sdk::{Deck, Error, ORC_F64, OrcHandle, OrcTypeId, Plugin, deck, handle_from_deck};
use std::path::Path;

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

fn main() -> Result<(), Error> {
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
        println!("{} function(s):", plugin.functions().len());
        for func in plugin.functions().iter() {
            println!("  - {}\n    {}", func.name, func.desc);
        }
    }
    // Test the add function.
    let math_plugin = &plugins[0];
    let add_fn = math_plugin.functions()[0].func;
    let a: Deck<f64> = deck![1.0, 2.0, 3.0];
    let b: Deck<f64> = deck![10.0, 20.0, 30.0];
    let mut out_handle = math_plugin.alloc_deck(OrcTypeId {
        primitive_id: ORC_F64,
        opaque_id: 0,
    })?;
    let inputs: &[OrcHandle] = &[handle_from_deck(&a, 0), handle_from_deck(&b, 1)];
    unsafe {
        add_fn(0, inputs.as_ptr(), inputs.len() as u64, &mut out_handle, 1);
    }
    // Print the output data.
    println!("Output deck: \n{}", out_handle.display::<f64>());
    math_plugin.free_deck(&mut out_handle)?;
    Ok(())
}
