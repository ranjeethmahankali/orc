use orc_sdk::PluginSet;
use pyo3::prelude::*;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn generate_stubs(py: Python<'_>, ps: &PluginSet) -> PyResult<()> {
    // Find the module's .so/.pyd file path to put the .pyi alongside it.
    let module = PyModule::import(py, "orc")?;
    let file_attr = match module.getattr("__file__") {
        Ok(f) => f.extract::<String>()?,
        Err(_) => return Ok(()), // No __file__ means we can't locate the stub path.
    };
    let module_path = PathBuf::from(&file_attr);
    let stub_path = module_path.with_extension("pyi");

    // Check if regeneration is needed: compare newest plugin mtime vs stub mtime.
    if let Ok(stub_meta) = std::fs::metadata(&stub_path) {
        if let Ok(stub_mtime) = stub_meta.modified() {
            let newest_plugin_mtime = newest_plugin_mtime(ps);
            if let Some(pm) = newest_plugin_mtime {
                if stub_mtime >= pm {
                    return Ok(()); // Stub is up to date.
                }
            }
        }
    }

    // Generate stub content.
    let content = build_stub_content(ps);

    // Write the file.
    let mut file = std::fs::File::create(&stub_path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Cannot write stub file: {}", e))
    })?;
    file.write_all(content.as_bytes()).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Cannot write stub file: {}", e))
    })?;

    Ok(())
}

fn newest_plugin_mtime(_ps: &PluginSet) -> Option<std::time::SystemTime> {
    // We don't have direct access to the plugin file paths from PluginSet.
    // Return None to always regenerate. This is conservative but correct.
    None
}

fn build_stub_content(ps: &PluginSet) -> String {
    let mut s = String::new();
    writeln!(s, "from typing import Callable").unwrap();
    writeln!(s).unwrap();
    // Constants
    writeln!(s, "ORC_TYPE_U8: int").unwrap();
    writeln!(s, "ORC_TYPE_U16: int").unwrap();
    writeln!(s, "ORC_TYPE_U32: int").unwrap();
    writeln!(s, "ORC_TYPE_U64: int").unwrap();
    writeln!(s, "ORC_TYPE_I8: int").unwrap();
    writeln!(s, "ORC_TYPE_I16: int").unwrap();
    writeln!(s, "ORC_TYPE_I32: int").unwrap();
    writeln!(s, "ORC_TYPE_I64: int").unwrap();
    writeln!(s, "ORC_TYPE_F32: int").unwrap();
    writeln!(s, "ORC_TYPE_F64: int").unwrap();
    writeln!(s).unwrap();
    // Handle class
    writeln!(s, "class Handle:").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def type_id(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def n_items(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def item_size(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def dims(self) -> list[int]: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def __array_interface__(self) -> dict: ...").unwrap();
    writeln!(s).unwrap();
    // GraphNode
    writeln!(s, "class GraphNode: ...").unwrap();
    writeln!(s).unwrap();
    // Graph
    writeln!(s, "class Graph:").unwrap();
    writeln!(
        s,
        "    def run(self, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )
    .unwrap();
    writeln!(s).unwrap();
    // Module functions
    writeln!(s, "def load_plugins(search_dir: str) -> None: ...").unwrap();
    writeln!(
        s,
        "def make_deck(data: object, type_id: int | None = None) -> Handle: ..."
    )
    .unwrap();
    writeln!(s, "def read_deck(handle: Handle) -> object: ...").unwrap();
    writeln!(s, "def make_graph(fn: Callable) -> Graph: ...").unwrap();
    writeln!(
        s,
        "def run_graph(graph: Graph, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )
    .unwrap();
    writeln!(
        s,
        "def serialize_workflow(graph: Graph, path: str) -> None: ..."
    )
    .unwrap();
    writeln!(s, "def deserialize_workflow(path: str) -> Graph: ...").unwrap();
    writeln!(s).unwrap();

    // Plugin functions
    writeln!(s, "# --- Plugin functions (auto-generated) ---").unwrap();
    for plugin in ps.plugins() {
        for func in plugin.functions() {
            let mut sig = String::new();
            write!(sig, "def {}(", func.name).unwrap();
            match func.n_inputs {
                Some(n) => {
                    let params: Vec<String> = (0..n).map(|i| format!("arg{}: Handle", i)).collect();
                    write!(sig, "{}", params.join(", ")).unwrap();
                }
                None => {
                    write!(sig, "*args: Handle").unwrap();
                }
            }
            write!(sig, ") -> ").unwrap();
            match func.n_outputs {
                Some(1) => write!(sig, "Handle").unwrap(),
                Some(n) if n > 1 => write!(sig, "list[Handle]").unwrap(),
                _ => write!(sig, "Handle | list[Handle]").unwrap(),
            }
            writeln!(sig, ": ...").unwrap();
            write!(s, "{}", sig).unwrap();
        }
    }

    s
}
