use orc_sdk::PluginSet;
use pyo3::prelude::*;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::PathBuf;

pub(crate) fn generate_stubs(py: Python<'_>, ps: &PluginSet) -> PyResult<()> {
    let module = PyModule::import(py, "orc")?;
    let file_attr = match module.getattr("__file__") {
        Ok(f) => f.extract::<String>()?,
        Err(_) => return Ok(()),
    };
    let stub_path = PathBuf::from(&file_attr).with_extension("pyi");
    let content = build_stub_content(ps);
    let mut file = std::fs::File::create(&stub_path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Cannot write stub file: {}", e))
    })?;
    file.write_all(content.as_bytes()).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Cannot write stub file: {}", e))
    })?;
    Ok(())
}

fn build_stub_content(ps: &PluginSet) -> String {
    let mut s = String::new();
    writeln!(s, "from typing import Callable").unwrap();
    writeln!(s).unwrap();
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
    writeln!(s, "class Handle:").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def type_id(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def n_items(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def item_size(self) -> int: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def dims(self) -> tuple[int, ...]: ...").unwrap();
    writeln!(s, "    @property").unwrap();
    writeln!(s, "    def __array_interface__(self) -> dict: ...").unwrap();
    writeln!(s).unwrap();
    writeln!(s, "class WorkflowNode: ...").unwrap();
    writeln!(s).unwrap();
    writeln!(s, "class Workflow:").unwrap();
    writeln!(
        s,
        "    def run(self, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )
    .unwrap();
    writeln!(s).unwrap();
    writeln!(s, "def load_plugins(search_dir: str) -> None: ...").unwrap();
    writeln!(
        s,
        "def make_deck(data: object, type_id: int | None = None) -> Handle: ..."
    )
    .unwrap();
    writeln!(s, "def read_deck(handle: Handle) -> object: ...").unwrap();
    writeln!(s, "def make_workflow(fn: Callable) -> Workflow: ...").unwrap();
    writeln!(
        s,
        "def run_workflow(graph: Workflow, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )
    .unwrap();
    writeln!(
        s,
        "def save_workflow(graph: Workflow, path: str) -> None: ..."
    )
    .unwrap();
    writeln!(s, "def load_workflow(path: str) -> Workflow: ...").unwrap();
    writeln!(s).unwrap();

    // Plugin functions — write directly into `s`, no intermediate string.
    writeln!(s, "# --- Plugin functions (auto-generated) ---").unwrap();
    for plugin in ps.plugins() {
        for func in plugin.functions() {
            write!(s, "def {}(", func.name).unwrap();
            match func.n_inputs {
                Some(n) => {
                    for i in 0..n {
                        if i > 0 {
                            write!(s, ", ").unwrap();
                        }
                        write!(s, "arg{}: Handle", i).unwrap();
                    }
                }
                None => write!(s, "*args: Handle").unwrap(),
            }
            write!(s, ") -> ").unwrap();
            match func.n_outputs {
                Some(1) => write!(s, "Handle").unwrap(),
                Some(n) if n > 1 => write!(s, "list[Handle]").unwrap(),
                _ => write!(s, "Handle | list[Handle]").unwrap(),
            }
            writeln!(s, ": ...").unwrap();
        }
    }

    s
}
