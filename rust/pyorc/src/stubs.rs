use orc_sdk::PluginSet;
use pyo3::prelude::*;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

pub(crate) fn generate_stubs(py: Python<'_>, ps: &PluginSet) -> PyResult<()> {
    let module = PyModule::import(py, "orc")?;
    let Ok(file_attr) = module.getattr("__file__") else {
        return Ok(());
    };
    let stub_path = PathBuf::from(file_attr.extract::<String>()?).with_extension("pyi");
    let io_err = |e| pyo3::exceptions::PyIOError::new_err(format!("Cannot write stub file: {}", e));
    let file = std::fs::File::create(&stub_path).map_err(&io_err)?;
    let mut w = BufWriter::new(file);
    write_stub_content(&mut w, ps).map_err(io_err)?;
    w.flush().map_err(io_err)
}

fn write_stub_content(w: &mut impl Write, ps: &PluginSet) -> std::io::Result<()> {
    writeln!(w, "from typing import Callable")?;
    writeln!(w)?;
    writeln!(w, "class Handle:")?;
    writeln!(w, "    @property")?;
    writeln!(w, "    def type_id(self) -> int: ...")?;
    writeln!(w, "    @property")?;
    writeln!(w, "    def n_items(self) -> int: ...")?;
    writeln!(w, "    @property")?;
    writeln!(w, "    def item_size(self) -> int: ...")?;
    writeln!(w, "    @property")?;
    writeln!(
        w,
        "    def dims(self) -> tuple[int, int, int, int, int, int, int]: ..."
    )?;
    writeln!(w, "    @property")?;
    writeln!(w, "    def __array_interface__(self) -> dict: ...")?;
    writeln!(w)?;
    writeln!(w, "class WorkflowNode: ...")?;
    writeln!(w)?;
    writeln!(w, "class Workflow:")?;
    writeln!(
        w,
        "    def run(self, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )?;
    writeln!(
        w,
        "    def has_nested_workflow(self, name: str) -> bool: ..."
    )?;
    writeln!(w, "    def count_nested_calls(self, name: str) -> int: ...")?;
    writeln!(w)?;
    writeln!(w, "class WorkflowFunc:")?;
    writeln!(
        w,
        "    def __call__(self, *args: WorkflowNode) -> WorkflowNode | list[WorkflowNode]: ..."
    )?;
    writeln!(w)?;
    writeln!(w, "def load_plugins(search_dir: str) -> None: ...")?;
    writeln!(
        w,
        "def make_deck(data: object, dtype: str | None = None) -> Handle: ..."
    )?;
    writeln!(w, "def read_deck(handle: Handle) -> object: ...")?;
    writeln!(w, "def make_workflow(fn: Callable) -> Workflow: ...")?;
    writeln!(
        w,
        "def run_workflow(graph: Workflow, *args: Handle, **kwargs: Handle) -> Handle | list[Handle]: ..."
    )?;
    writeln!(
        w,
        "def save_workflow(graph: Workflow, path: str) -> None: ..."
    )?;
    writeln!(w, "def load_workflow(path: str) -> Workflow: ...")?;
    writeln!(
        w,
        "def workflow_function(func: Callable) -> WorkflowFunc: ..."
    )?;
    writeln!(w)?;
    writeln!(w, "# --- Plugin functions (auto-generated) ---")?;
    for plugin in ps.plugins() {
        for func in plugin.functions() {
            write!(w, "def {}(", func.name)?;
            match func.n_inputs {
                Some(n) => {
                    for i in 0..n {
                        if i > 0 {
                            write!(w, ", ")?;
                        }
                        write!(w, "arg{}: Handle", i)?;
                    }
                }
                None => write!(w, "*args: Handle")?,
            }
            write!(w, ") -> ")?;
            match func.n_outputs {
                Some(1) => write!(w, "Handle")?,
                Some(n) if n > 1 => write!(w, "list[Handle]")?,
                _ => write!(w, "Handle | list[Handle]")?,
            }
            writeln!(w, ": ...")?;
        }
    }
    Ok(())
}
