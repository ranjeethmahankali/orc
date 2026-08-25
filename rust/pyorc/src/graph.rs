use crate::handle::Handle;
use crate::host::{
    HANDLE_COUNTER, PLUGIN_SET, REGISTRY, SERIAL_CONTEXT_ARENA, host_clone_orc_handle,
};
use orc_sdk::{DagHandle, IH, OH, OrcHandle, OrcHandleBorrowed, Workflow};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct WorkflowBuildState {
    pub(crate) workflow: SendWorkflow,
    pub(crate) inputs: Vec<(IH, usize)>,
    /// Maps nested workflow name to n_outputs.
    /// Used to avoid building the same nested workflow more than once per outer workflow.
    pub(crate) nested_built: HashMap<String, usize>,
}

pub(crate) static IN_WORKFLOW_MODE: AtomicBool = AtomicBool::new(false);
pub(crate) static BUILDING_WORKFLOW: Mutex<Vec<WorkflowBuildState>> = Mutex::new(Vec::new());

/// Wrapper to make Workflow Send+Sync (required by #[pyclass] and Mutex statics).
/// SAFETY: Workflow contains Rc<RefCell<...>> (property system) which is !Send !Sync.
/// However, the Workflow is only ever accessed while holding the Python GIL, so no
/// concurrent access occurs.
#[repr(transparent)]
pub(crate) struct SendWorkflow(pub Workflow);
unsafe impl Send for SendWorkflow {}
unsafe impl Sync for SendWorkflow {}

impl std::ops::Deref for SendWorkflow {
    type Target = Workflow;
    fn deref(&self) -> &Workflow {
        &self.0
    }
}

impl std::ops::DerefMut for SendWorkflow {
    fn deref_mut(&mut self) -> &mut Workflow {
        &mut self.0
    }
}

#[derive(Clone, Copy)]
pub(crate) enum WorkflowNodeKind {
    UpstreamNode(OH),
    WorkflowInput(usize),
}

/// WorkflowNodeKind cannot be a #[pyclass] directly because PyO3's enum pyclass support is limited
/// to unit variants. This struct exists solely as the pyclass boundary; WorkflowNodeKind stays a
/// plain Copy enum for cheap internal use.
#[pyclass(name = "WorkflowNode")]
pub(crate) struct WorkflowNode {
    pub(crate) kind: WorkflowNodeKind,
}

#[pymethods]
impl WorkflowNode {
    fn __repr__(&self) -> String {
        match self.kind {
            WorkflowNodeKind::UpstreamNode(oh) => format!("<Workflow output={}>", oh.index()),
            WorkflowNodeKind::WorkflowInput(idx) => format!("<Workflow input={}>", idx),
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
        // Collect positional args, pad remaining slots with None.
        let n_params = self.param_names.len();
        if args.len() > n_params {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "Too many positional arguments",
            ));
        }
        let mut ordered: Vec<Option<Py<Handle>>> = args
            .iter()
            .map(|arg| arg.extract())
            .chain(std::iter::repeat_with(|| Ok(None)))
            .take(n_params)
            .collect::<PyResult<Vec<_>>>()?;
        // Fill in keyword args.
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
                if ordered[idx].replace(value.extract()?).is_some() {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Duplicate argument: {}",
                        name
                    )));
                }
            }
        }
        // Borrow input handles. Missing args become empty default handles —
        // Workflow::run passes those through to the plugin functions.
        let refs: Vec<Option<PyRef<'py, Handle>>> = ordered
            .iter()
            .map(|slot| slot.as_ref().map(|h| h.bind(py).borrow()))
            .collect();
        let empty = OrcHandle::default();
        let input_borrows: Vec<OrcHandleBorrowed<'_>> = refs
            .iter()
            .map(|r| match r {
                Some(h) => h.inner.borrowed(),
                None => empty.borrowed(),
            })
            .collect();
        // Allocate output handles.
        let n_outputs = self.workflow.workflow_outputs().len();
        let base_id = HANDLE_COUNTER.fetch_add(n_outputs as u64, Ordering::Relaxed);
        let mut outputs: Vec<OrcHandle> = (0..n_outputs)
            .map(|i| OrcHandle {
                handle: base_id + i as u64,
                ..Default::default()
            })
            .collect();
        // Run the workflow.
        self.workflow
            .run(
                &input_borrows,
                &mut outputs,
                &host_clone_orc_handle,
                &HANDLE_COUNTER,
            )
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        // Wrap outputs.
        if let Some(last) = outputs.pop() {
            if outputs.is_empty() {
                return Ok(Py::new(py, Handle::new(last))?.into_any());
            } else {
                outputs.push(last);
            }
        }
        Ok(PyList::new(
            py,
            outputs
                .into_iter()
                .map(|h| Py::new(py, Handle::new(h)))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any()
        .unbind())
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

    fn has_nested_workflow(&self, name: &str) -> bool {
        self.workflow.has_nested_workflow(name)
    }

    fn count_nested_calls(&self, name: &str) -> PyResult<usize> {
        self.workflow
            .count_nested_calls(name)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
    }
}

// =====================================================================
// make_workflow
// =====================================================================

/// Finalizes a WorkflowBuildState: sets workflow inputs and outputs, returns n_outputs.
/// Called while the BUILDING_WORKFLOW lock is held.
fn finalize_build_state(
    state: &mut WorkflowBuildState,
    return_value: &Bound<'_, PyAny>,
    param_names: &[String],
) -> PyResult<usize> {
    // Destructure to get independent borrows of workflow and inputs.
    let WorkflowBuildState {
        workflow, inputs, ..
    } = state;
    let wf: &mut Workflow = workflow;
    let input_refs: Vec<(IH, usize, &str)> = inputs
        .drain(..)
        .map(|(ih, idx)| -> PyResult<(IH, usize, &str)> {
            let name = param_names.get(idx).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Workflow input index {} out of range (expected < {})",
                    idx,
                    param_names.len()
                ))
            })?;
            Ok((ih, idx, name.as_str()))
        })
        .collect::<PyResult<_>>()?;
    if !input_refs.is_empty() {
        wf.set_inputs(&input_refs)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    }
    let output_ohs = extract_output_ohs(return_value)?;
    let n_outputs = output_ohs.len();
    let outputs: Vec<(OH, String)> = output_ohs.iter().map(|&oh| (oh, String::new())).collect();
    wf.set_outputs(&outputs)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
    Ok(n_outputs)
}

pub(crate) fn make_workflow_impl(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<PyWorkflow> {
    // Prevent recursive calls to make_workflow at the top level.
    IN_WORKFLOW_MODE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| {
            pyo3::exceptions::PyRuntimeError::new_err("make_workflow cannot be called recursively.")
        })?;
    // Drop guard resets state on any exit path (panic, error, success).
    struct WorkflowModeGuard;
    impl Drop for WorkflowModeGuard {
        fn drop(&mut self) {
            IN_WORKFLOW_MODE.store(false, Ordering::SeqCst);
            if let Ok(mut stack) = BUILDING_WORKFLOW.lock() {
                stack.clear();
            }
        }
    }
    let _guard = WorkflowModeGuard;
    {
        // Push the top-level workflow frame.
        let mut stack = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error"))?;
        stack.push(WorkflowBuildState {
            workflow: SendWorkflow(Workflow::default()),
            inputs: Vec::new(),
            nested_built: HashMap::new(),
        });
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
    // Create WorkflowNode::WorkflowInput for each parameter.
    let graph_nodes: Vec<Py<WorkflowNode>> = param_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::WorkflowInput(i),
                },
            )
        })
        .collect::<PyResult<_>>()?;
    // Call the user function in deferred mode.
    let py_args = PyTuple::new(py, &graph_nodes)?;
    let return_value = func.call1(&py_args)?;
    // Finalize: set workflow inputs and outputs, then pop the finished frame.
    // The block scope releases the BUILDING_WORKFLOW lock before we reset IN_WORKFLOW_MODE.
    let workflow = {
        let mut stack = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error"))?;
        {
            let state = stack.last_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("No workflow being built")
            })?;
            finalize_build_state(state, &return_value, &param_names)?;
        }
        stack
            .pop()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?
            .workflow
    }; // BUILDING_WORKFLOW lock released here.
    IN_WORKFLOW_MODE.store(false, Ordering::SeqCst);
    // _guard still runs on drop as a safety net for error/panic paths.
    Ok(PyWorkflow {
        param_names,
        workflow,
    })
}

/// Build a nested workflow by calling `func` with synthetic WorkflowInput nodes.
/// Called while `IN_WORKFLOW_MODE` is already true. Pushes a new frame onto the
/// BUILDING_WORKFLOW stack, runs the function, finalizes the workflow, and pops
/// the frame via a drop guard.
pub(crate) fn build_nested_workflow_impl(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
    param_names: &[String],
) -> PyResult<(SendWorkflow, usize)> {
    // Push a new frame for the nested build.
    {
        let mut stack = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
        stack.push(WorkflowBuildState {
            workflow: SendWorkflow(Workflow::default()),
            inputs: Vec::new(),
            nested_built: HashMap::new(),
        });
    }
    // Guard that pops the frame on any exit path.
    struct NestedFrameGuard;
    impl Drop for NestedFrameGuard {
        fn drop(&mut self) {
            if let Ok(mut stack) = BUILDING_WORKFLOW.lock() {
                stack.pop();
            }
        }
    }
    let _guard = NestedFrameGuard;
    // Create WorkflowInput nodes for each parameter.
    let graph_nodes: Vec<Py<WorkflowNode>> = param_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::WorkflowInput(i),
                },
            )
        })
        .collect::<PyResult<_>>()?;
    let py_args = PyTuple::new(py, &graph_nodes)?;
    // Call the user function; this may recursively build deeper nested workflows.
    let return_value = func.call1(&py_args)?;
    // Finalize the nested workflow.
    let (workflow, n_outputs) = {
        let mut stack = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
        let state = stack
            .last_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow frame"))?;
        let n_outputs = finalize_build_state(state, &return_value, param_names)?;
        let workflow = std::mem::replace(&mut state.workflow, SendWorkflow(Workflow::default()));
        (workflow, n_outputs)
    }; // Lock released; NestedFrameGuard pops the frame on drop.
    Ok((workflow, n_outputs))
}

// =====================================================================
// Serialization
// =====================================================================

pub(crate) fn save_workflow_impl(graph: &PyWorkflow, path: &str) -> PyResult<()> {
    let ps = PLUGIN_SET
        .lock()
        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error."))?;
    let file = std::fs::File::create(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut writer = std::io::BufWriter::new(file);
    graph
        .workflow
        .write_to_msgpack(&ps, &SERIAL_CONTEXT_ARENA, &mut writer)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
}

pub(crate) fn load_workflow_impl(path: &str) -> PyResult<PyWorkflow> {
    let ps = PLUGIN_SET
        .lock()
        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error."))?;
    let file = std::fs::File::open(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let wf = Workflow::read_from_msgpack(&mut reader, &ps, &REGISTRY, 0, &mut next_id)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    Ok(PyWorkflow {
        param_names: wf.input_names().to_vec(),
        workflow: SendWorkflow(wf),
    })
}

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
        WorkflowNodeKind::UpstreamNode(oh) => Ok(oh),
        WorkflowNodeKind::WorkflowInput(_) => Err(pyo3::exceptions::PyTypeError::new_err(
            "make_workflow function must return function call results, not raw inputs.",
        )),
    }
}
