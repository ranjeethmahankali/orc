use crate::handle::Handle;
use crate::host::{
    HANDLE_COUNTER, PLUGIN_SET, REGISTRY, SERIAL_CONTEXT_ARENA, host_clone_orc_handle,
};
use orc_sdk::{DagHandle, IH, OH, OrcHandle, OrcHandleBorrowed, Workflow};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::mem::ManuallyDrop;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) static IN_GRAPH_MODE: AtomicBool = AtomicBool::new(false);
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

/// What a GraphNode represents.
#[derive(Clone, Copy)]
pub(crate) enum GraphNodeKind {
    /// An output of a function call or a constant node in the DAG.
    Output(OH),
    /// A workflow input, identified by parameter index.
    Input(usize),
}

#[pyclass(name = "GraphNode")]
pub(crate) struct GraphNode {
    pub(crate) kind: GraphNodeKind,
}

// GraphNodeKind is Copy (OH is Copy, usize is Copy).
unsafe impl Send for GraphNode {}
unsafe impl Sync for GraphNode {}

#[pymethods]
impl GraphNode {
    fn __repr__(&self) -> String {
        match self.kind {
            GraphNodeKind::Output(oh) => format!("<GraphNode output={}>", oh.index()),
            GraphNodeKind::Input(idx) => format!("<GraphNode input={}>", idx),
        }
    }
}

#[pyclass(name = "Graph")]
pub(crate) struct Graph {
    pub(crate) workflow: SendWorkflow,
    pub(crate) param_names: Vec<String>,
}

impl Graph {
    pub(crate) fn run_impl<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyObject> {
        let n_params = self.param_names.len();
        let mut ordered: Vec<Option<Py<Handle>>> = Vec::new();
        ordered.resize_with(n_params, || None);

        for (i, arg) in args.iter().enumerate() {
            if i >= n_params {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "too many positional arguments",
                ));
            }
            ordered[i] = Some(arg.extract()?);
        }

        if let Some(kw) = kwargs {
            for (key, value) in kw.iter() {
                let name: String = key.extract()?;
                let idx = self
                    .param_names
                    .iter()
                    .position(|n| n == &name)
                    .ok_or_else(|| {
                        pyo3::exceptions::PyValueError::new_err(format!(
                            "unknown parameter: {}",
                            name
                        ))
                    })?;
                if ordered[idx].is_some() {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "duplicate argument: {}",
                        name
                    )));
                }
                ordered[idx] = Some(value.extract()?);
            }
        }

        for (i, slot) in ordered.iter().enumerate() {
            if slot.is_none() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "missing argument: {}",
                    self.param_names[i]
                )));
            }
        }

        // Build contiguous borrowed-input array.
        let mut raw_inputs: Vec<ManuallyDrop<OrcHandle>> = Vec::with_capacity(n_params);
        for slot in &ordered {
            let py_handle = slot.as_ref().unwrap();
            let h = py_handle.bind(py).borrow();
            let src = &h.inner.0;
            raw_inputs.push(ManuallyDrop::new(OrcHandle {
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
            }));
        }

        // SAFETY: ManuallyDrop<OrcHandle> is layout-identical to OrcHandle.
        // OrcHandleBorrowed is repr(transparent) over OrcHandle.
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
impl Graph {
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
// make_graph
// =====================================================================

/// Build a workflow DAG by running `func` in deferred mode.
pub(crate) fn make_graph_impl(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<Graph> {
    if IN_GRAPH_MODE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "make_graph cannot be called recursively",
        ));
    }

    // Drop guard ensures cleanup on any exit path (panic, error, success).
    struct GraphModeGuard;
    impl Drop for GraphModeGuard {
        fn drop(&mut self) {
            IN_GRAPH_MODE.store(false, Ordering::SeqCst);
            if let Ok(mut wf) = BUILDING_WORKFLOW.lock() {
                *wf = None;
            }
            if let Ok(mut wi) = WORKFLOW_INPUTS.lock() {
                wi.clear();
            }
        }
    }
    let _guard = GraphModeGuard;

    {
        let mut wf = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("lock error"))?;
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

    // Create GraphNode::Input for each parameter — pure graph concept, no OrcHandle.
    let graph_nodes: Vec<Py<GraphNode>> = param_names
        .iter()
        .enumerate()
        .map(|(i, _)| {
            Py::new(
                py,
                GraphNode {
                    kind: GraphNodeKind::Input(i),
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
    let mut output_ohs: Vec<OH> = Vec::new();
    if let Ok(node) = return_value.extract::<PyRef<'_, GraphNode>>() {
        match node.kind {
            GraphNodeKind::Output(oh) => output_ohs.push(oh),
            GraphNodeKind::Input(_) => {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "make_graph function must return function call results, not raw inputs",
                ));
            }
        }
    } else if let Ok(tuple) = return_value.downcast::<PyTuple>() {
        for item in tuple.iter() {
            let node: PyRef<'_, GraphNode> = item.extract()?;
            match node.kind {
                GraphNodeKind::Output(oh) => output_ohs.push(oh),
                GraphNodeKind::Input(_) => {
                    return Err(pyo3::exceptions::PyTypeError::new_err(
                        "make_graph function must return function call results, not raw inputs",
                    ));
                }
            }
        }
    } else if let Ok(list) = return_value.downcast::<PyList>() {
        for item in list.iter() {
            let node: PyRef<'_, GraphNode> = item.extract()?;
            match node.kind {
                GraphNodeKind::Output(oh) => output_ohs.push(oh),
                GraphNodeKind::Input(_) => {
                    return Err(pyo3::exceptions::PyTypeError::new_err(
                        "make_graph function must return function call results, not raw inputs",
                    ));
                }
            }
        }
    } else {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "make_graph function must return GraphNode or a list/tuple of GraphNodes",
        ));
    }

    let outputs: Vec<(OH, String)> = output_ohs
        .into_iter()
        .map(|oh| (oh, String::new()))
        .collect();
    wf.set_outputs(&outputs)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;

    let workflow = wf_guard.take().unwrap();

    Ok(Graph {
        workflow,
        param_names,
    })
}

// =====================================================================
// Serialization
// =====================================================================

pub(crate) fn serialize_workflow_impl(graph: &Graph, path: &str) -> PyResult<()> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("plugins not loaded"))?;
    let file = std::fs::File::create(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut writer = std::io::BufWriter::new(file);
    graph
        .workflow
        .0
        .write_to_msgpack(ps, &SERIAL_CONTEXT_ARENA, &mut writer)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))
}

pub(crate) fn deserialize_workflow_impl(_py: Python<'_>, path: &str) -> PyResult<Graph> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("plugins not loaded"))?;
    let file = std::fs::File::open(path)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("{}", e)))?;
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let wf = Workflow::read_from_msgpack(&mut reader, ps, &REGISTRY, 0, &mut next_id)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;
    let param_names = wf.input_names().to_vec();
    Ok(Graph {
        workflow: SendWorkflow(wf),
        param_names,
    })
}
