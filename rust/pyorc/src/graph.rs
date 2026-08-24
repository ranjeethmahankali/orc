use crate::handle::Handle;
use crate::host::{
    HANDLE_COUNTER, PLUGIN_SET, REGISTRY, SERIAL_CONTEXT_ARENA, host_clone_orc_handle,
};
use orc_sdk::{DagHandle, IH, OH, OrcHandle, OrcHandleBorrowed, Workflow};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) static IN_WORKFLOW_MODE: AtomicBool = AtomicBool::new(false);
pub(crate) static BUILDING_WORKFLOW: Mutex<Option<SendWorkflow>> = Mutex::new(None);
/// Accumulated workflow input mappings: (IH, param_index).
pub(crate) static WORKFLOW_INPUTS: Mutex<Vec<(IH, usize)>> = Mutex::new(Vec::new());

/// Wrapper to make Workflow Send+Sync (required by #[pyclass] and Mutex statics).
/// SAFETY: Workflow contains Rc<RefCell<...>> (property system) which is !Send !Sync.
/// However, the Workflow is only ever accessed while holding the Python GIL, so no
/// concurrent access occurs.
pub(crate) struct SendWorkflow(pub Workflow);
unsafe impl Send for SendWorkflow {}
unsafe impl Sync for SendWorkflow {}

/// What a WorkflowNode represents.
#[derive(Clone, Copy)]
pub(crate) enum WorkflowNodeKind {
    /// An output of a function call or a constant node in the DAG.
    Output(OH),
    /// A workflow input, identified by parameter index.
    Input(usize),
}

#[pyclass(name = "WorkflowNode")]
pub(crate) struct WorkflowNode {
    pub(crate) kind: WorkflowNodeKind,
}

// WorkflowNodeKind is Copy (OH is Copy, usize is Copy).
unsafe impl Send for WorkflowNode {}
unsafe impl Sync for WorkflowNode {}

#[pymethods]
impl WorkflowNode {
    fn __repr__(&self) -> String {
        match self.kind {
            WorkflowNodeKind::Output(oh) => format!("<Workflow output={}>", oh.index()),
            WorkflowNodeKind::Input(idx) => format!("<Workflow input={}>", idx),
        }
    }
}

#[pyclass(name = "Workflow")]
pub(crate) struct PyWorkflow {
    pub(crate) workflow: SendWorkflow,
    pub(crate) param_names: Vec<String>,
}

impl PyWorkflow {
    pub(crate) fn run_impl<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyObject> {
        let n_params = self.param_names.len();
        if args.len() > n_params {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Too many positional arguments",
            ));
        }
        let mut ordered: Vec<Option<Py<Handle>>> = args
            .iter()
            .map(|arg| arg.extract())
            .collect::<PyResult<Vec<_>>>()?;
        ordered.resize_with(n_params, || None);

        if let Some(kw) = kwargs {
            for (key, value) in kw.iter() {
                let name: String = key.extract()?;
                let idx = self
                    .param_names
                    .iter()
                    .position(|n| n == &name)
                    .ok_or_else(|| {
                        pyo3::exceptions::PyValueError::new_err(format!(
                            "Unknown parameter: {}",
                            name
                        ))
                    })?;
                if ordered[idx].is_some() {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Duplicate argument: {}",
                        name
                    )));
                }
                ordered[idx] = Some(value.extract()?);
            }
        }

        for (i, slot) in ordered.iter().enumerate() {
            if slot.is_none() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Missing argument: {}",
                    self.param_names[i]
                )));
            }
        }

        // Build contiguous borrowed-input array (free_fn = None → drop is a no-op).
        let mut raw_inputs: Vec<OrcHandle> = Vec::with_capacity(n_params);
        for slot in &ordered {
            let py_handle = slot.as_ref().unwrap();
            let h = py_handle.bind(py).borrow();
            let src = &h.inner.0;
            raw_inputs.push(OrcHandle {
                handle: src.handle,
                type_id: src.type_id,
                dims: src.dims,
                n_items: src.n_items,
                item_size: src.item_size,
                n_marks: src.n_marks,
                free_fn: None,
                marks: src.marks,
                stride_offset: src.stride_offset,
                strides: src.strides,
                items: src.items,
            });
        }

        // SAFETY: OrcHandleBorrowed is repr(transparent) over OrcHandle.
        let input_borrows: &[OrcHandleBorrowed<'_>] = unsafe {
            std::slice::from_raw_parts(
                raw_inputs.as_ptr() as *const OrcHandleBorrowed<'_>,
                raw_inputs.len(),
            )
        };

        let n_outputs = self.workflow.0.workflow_outputs().len();
        let base_id = HANDLE_COUNTER.fetch_add(n_outputs as u64, Ordering::Relaxed);
        let mut outputs: Vec<OrcHandle> = (0..n_outputs)
            .map(|i| OrcHandle {
                handle: base_id + i as u64,
                ..Default::default()
            })
            .collect();

        self.workflow
            .0
            .run(
                input_borrows,
                &mut outputs,
                &host_clone_orc_handle,
                &HANDLE_COUNTER,
            )
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;

        if n_outputs == 1 {
            Ok(Py::new(py, Handle::new(outputs.pop().unwrap()))?.into_any())
        } else {
            let list = PyList::empty(py);
            for h in outputs {
                list.append(Py::new(py, Handle::new(h))?)?;
            }
            Ok(list.into_any().unbind())
        }
    }
}

#[pymethods]
impl PyWorkflow {
    #[pyo3(signature = (*args, **kwargs))]
    fn run<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyObject> {
        self.run_impl(py, args, kwargs)
    }
}

// =====================================================================
// make_workflow
// =====================================================================

/// Build a workflow DAG by running `func` in deferred mode.
pub(crate) fn make_workflow_impl(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<PyWorkflow> {
    if IN_WORKFLOW_MODE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "make_workflow cannot be called recursively.",
        ));
    }

    // Drop guard ensures cleanup on any exit path (panic, error, success).
    struct WorkflowModeGuard;
    impl Drop for WorkflowModeGuard {
        fn drop(&mut self) {
            IN_WORKFLOW_MODE.store(false, Ordering::SeqCst);
            if let Ok(mut wf) = BUILDING_WORKFLOW.lock() {
                *wf = None;
            }
            if let Ok(mut wi) = WORKFLOW_INPUTS.lock() {
                wi.clear();
            }
        }
    }
    let _guard = WorkflowModeGuard;

    {
        let mut wf = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error"))?;
        *wf = Some(SendWorkflow(Workflow::default()));
    }

    // Get parameter names via inspect.signature.
    let inspect = py.import("inspect")?;
    let sig = inspect.call_method1("signature", (func,))?;
    let params = sig.getattr("parameters")?;
    let param_names: Vec<String> = params
        .call_method0("keys")?
        .try_iter()?
        .map(|k| k.and_then(|k| k.extract::<String>()))
        .collect::<PyResult<_>>()?;

    // Create WorkflowNode::Input for each parameter — pure graph concept, no OrcHandle.
    let graph_nodes: Vec<Py<WorkflowNode>> = param_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::Input(i),
                },
            )
        })
        .collect::<PyResult<_>>()?;

    // Call the user function.
    let py_args = PyTuple::new(py, &graph_nodes)?;
    let return_value = func.call1(&py_args)?;

    // Finalize: set workflow inputs and outputs.
    let mut wf_guard = BUILDING_WORKFLOW.lock().unwrap();
    let wf = &mut wf_guard.as_mut().unwrap().0;

    // Collect accumulated input mappings from deferred calls.
    let input_map: Vec<(IH, usize)> = WORKFLOW_INPUTS.lock().unwrap().drain(..).collect();
    if !input_map.is_empty() {
        let refs: Vec<(IH, usize, &str)> = input_map
            .iter()
            .map(|(ih, idx)| (*ih, *idx, param_names[*idx].as_str()))
            .collect();
        wf.set_inputs(&refs)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;
    }

    // Extract workflow outputs from the return value.
    let output_ohs = extract_output_ohs(&return_value)?;
    let outputs: Vec<(OH, String)> = output_ohs.iter().map(|&oh| (oh, String::new())).collect();
    wf.set_outputs(&outputs)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;

    let workflow = wf_guard.take().unwrap();

    Ok(PyWorkflow {
        workflow,
        param_names,
    })
}

// =====================================================================
// Serialization
// =====================================================================

pub(crate) fn save_workflow_impl(graph: &PyWorkflow, path: &str) -> PyResult<()> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Plugins not loaded"))?;
    let file = std::fs::File::create(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut writer = std::io::BufWriter::new(file);
    graph
        .workflow
        .0
        .write_to_msgpack(ps, &SERIAL_CONTEXT_ARENA, &mut writer)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
}

pub(crate) fn load_workflow_impl(_py: Python<'_>, path: &str) -> PyResult<PyWorkflow> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Plugins not loaded"))?;
    let file = std::fs::File::open(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let wf = Workflow::read_from_msgpack(&mut reader, ps, &REGISTRY, 0, &mut next_id)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;
    let param_names = wf.input_names().to_vec();
    Ok(PyWorkflow {
        workflow: SendWorkflow(wf),
        param_names,
    })
}

/// Extract output OHs from the return value of a user's workflow function.
/// Accepts a single WorkflowNode, or a tuple/list of them.
fn extract_output_ohs(value: &Bound<'_, PyAny>) -> PyResult<Vec<OH>> {
    // Single WorkflowNode.
    if let Ok(node) = value.extract::<PyRef<'_, WorkflowNode>>() {
        return Ok(vec![require_output_oh(&node)?]);
    }
    // Tuple or list of WorkflowNodes.
    let items: Vec<PyRef<'_, WorkflowNode>> = value.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "make_workflow function must return a WorkflowNode or a list/tuple of WorkflowNodes.",
        )
    })?;
    items.iter().map(|n| require_output_oh(n)).collect()
}

fn require_output_oh(node: &WorkflowNode) -> PyResult<OH> {
    match node.kind {
        WorkflowNodeKind::Output(oh) => Ok(oh),
        WorkflowNodeKind::Input(_) => Err(pyo3::exceptions::PyTypeError::new_err(
            "make_workflow function must return function call results, not raw inputs.",
        )),
    }
}
