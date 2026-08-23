use crate::handle::Handle;
use crate::host::{HANDLE_COUNTER, PLUGIN_SET, REGISTRY, SERIAL_CONTEXT_ARENA, host_clone_orc_handle};
use orc_sdk::{DagHandle, IH, OH, OrcHandle, OrcHandleBorrowed, Workflow};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub(crate) static IN_GRAPH_MODE: AtomicBool = AtomicBool::new(false);
pub(crate) static BUILDING_WORKFLOW: Mutex<Option<SendWorkflow>> = Mutex::new(None);

/// Wrapper to make Workflow Send+Sync (required by #[pyclass]).
/// SAFETY: Workflow contains Rc<RefCell<...>> (property system) which is !Send !Sync.
/// However, the Workflow is only ever accessed while holding the Python GIL, so no
/// concurrent access occurs. PyO3 enforces GIL acquisition for all method calls.
pub(crate) struct SendWorkflow(pub Workflow);
unsafe impl Send for SendWorkflow {}
unsafe impl Sync for SendWorkflow {}

#[pyclass(name = "GraphNode")]
pub(crate) struct GraphNode {
    pub(crate) oh: OH,
}

// OH contains only a usize, which is Send+Sync.
unsafe impl Send for GraphNode {}
unsafe impl Sync for GraphNode {}

#[pymethods]
impl GraphNode {
    fn __repr__(&self) -> String {
        format!("<GraphNode idx={}>", self.oh.index())
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

        // Collect input handles — use Py<Handle> so we don't need Clone on PyRef.
        let mut ordered: Vec<Option<Py<Handle>>> = Vec::new();
        ordered.resize_with(n_params, || None);

        // Process positional args.
        for (i, arg) in args.iter().enumerate() {
            if i >= n_params {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "too many positional arguments",
                ));
            }
            ordered[i] = Some(arg.extract()?);
        }

        // Process keyword args.
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

        // Ensure all inputs are provided.
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

        // Prepare outputs.
        let n_outputs = self.workflow.0.workflow_outputs().len();
        let base_id = HANDLE_COUNTER.fetch_add(n_outputs as u64, Ordering::Relaxed);
        let mut outputs: Vec<OrcHandle> = (0..n_outputs)
            .map(|i| OrcHandle {
                handle: base_id + i as u64,
                ..Default::default()
            })
            .collect();

        // Run the workflow.
        self.workflow
            .0
            .run(
                input_borrows,
                &mut outputs,
                &host_clone_orc_handle,
                &HANDLE_COUNTER,
            )
            .map_err(|e| {
                pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
            })?;

        // Wrap outputs.
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

/// Build a workflow DAG by running `func` in deferred mode.
pub(crate) fn make_graph_impl(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<Graph> {
    // 1. Enter graph mode.
    if IN_GRAPH_MODE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "make_graph cannot be called recursively",
        ));
    }

    // Guard that resets IN_GRAPH_MODE and cleans BUILDING_WORKFLOW on any exit path.
    struct GraphModeGuard;
    impl Drop for GraphModeGuard {
        fn drop(&mut self) {
            IN_GRAPH_MODE.store(false, Ordering::SeqCst);
            if let Ok(mut wf) = BUILDING_WORKFLOW.lock() {
                *wf = None;
            }
        }
    }
    let _guard = GraphModeGuard;

    // 2. Create a fresh workflow.
    {
        let mut wf = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("lock error"))?;
        *wf = Some(SendWorkflow(Workflow::default()));
    }

    // 3. Get parameter names via inspect.signature.
    let inspect = py.import("inspect")?;
    let sig = inspect.call_method1("signature", (func,))?;
    let params = sig.getattr("parameters")?;
    let param_names: Vec<String> = params
        .call_method0("keys")?
        .try_iter()?
        .map(|k| k.and_then(|k| k.extract::<String>()))
        .collect::<PyResult<_>>()?;

    // 4. Create placeholder constant nodes for each parameter and wrap as GraphNode.
    let graph_nodes: Vec<Py<GraphNode>> = {
        let mut wf_guard = BUILDING_WORKFLOW.lock().unwrap();
        let wf = &mut wf_guard.as_mut().unwrap().0;
        let mut nodes = Vec::with_capacity(param_names.len());
        for _name in &param_names {
            let (_nh, oh) = wf
                .add_constant(OrcHandle::default())
                .map_err(|e| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
                })?;
            nodes.push(Py::new(py, GraphNode { oh })?);
        }
        nodes
    };

    // Collect OHs for later use.
    let param_ohs: Vec<OH> = graph_nodes
        .iter()
        .map(|n| n.bind(py).borrow().oh)
        .collect();

    // 5. Call the user function with GraphNode arguments.
    let py_args = PyTuple::new(py, &graph_nodes)?;
    let return_value = func.call1(&py_args)?;

    // 6. Process results: extract outputs, disconnect param links, set workflow inputs.
    let mut wf_guard = BUILDING_WORKFLOW.lock().unwrap();
    let wf = &mut wf_guard.as_mut().unwrap().0;

    // Find all IHs connected to parameter-node OHs and disconnect them,
    // recording them as workflow inputs.
    let mut workflow_inputs: Vec<(IH, usize, String)> = Vec::new();
    for (param_idx, param_oh) in param_ohs.iter().enumerate() {
        let n_inputs = wf.num_total_inputs();
        for ih_idx in 0..n_inputs {
            let ih = IH::from(ih_idx);
            if let Some(src_oh) = wf.input_source(ih) {
                if src_oh == *param_oh {
                    wf.disconnect(*param_oh, ih);
                    workflow_inputs.push((ih, param_idx, param_names[param_idx].clone()));
                }
            }
        }
    }

    if !workflow_inputs.is_empty() {
        let refs: Vec<(IH, usize, &str)> = workflow_inputs
            .iter()
            .map(|(ih, idx, name)| (*ih, *idx, name.as_str()))
            .collect();
        wf.set_inputs(&refs).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
        })?;
    }

    // Extract workflow outputs from the return value.
    let mut output_ohs: Vec<OH> = Vec::new();
    if let Ok(node) = return_value.extract::<PyRef<'_, GraphNode>>() {
        output_ohs.push(node.oh);
    } else if let Ok(tuple) = return_value.downcast::<PyTuple>() {
        for item in tuple.iter() {
            let node: PyRef<'_, GraphNode> = item.extract()?;
            output_ohs.push(node.oh);
        }
    } else if let Ok(list) = return_value.downcast::<PyList>() {
        for item in list.iter() {
            let node: PyRef<'_, GraphNode> = item.extract()?;
            output_ohs.push(node.oh);
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
    wf.set_outputs(&outputs).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
    })?;

    // 7. Take the workflow out.
    let workflow = wf_guard.take().unwrap();

    Ok(Graph {
        workflow,
        param_names,
    })
}

/// Serialize a workflow to a msgpack file.
pub(crate) fn serialize_workflow_impl(graph: &Graph, path: &str) -> PyResult<()> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("plugins not loaded"))?;
    let file = std::fs::File::create(path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("{}", e))
    })?;
    let mut writer = std::io::BufWriter::new(file);
    graph
        .workflow
        .0
        .write_to_msgpack(ps, &SERIAL_CONTEXT_ARENA, &mut writer)
        .map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
        })
}

/// Deserialize a workflow from a msgpack file.
pub(crate) fn deserialize_workflow_impl(_py: Python<'_>, path: &str) -> PyResult<Graph> {
    let ps = PLUGIN_SET
        .get()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("plugins not loaded"))?;
    let file = std::fs::File::open(path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("{}", e))
    })?;
    let mut reader = std::io::BufReader::new(file);
    let mut next_id = || HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let wf = Workflow::read_from_msgpack(&mut reader, ps, &REGISTRY, 0, &mut next_id)
        .map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
        })?;

    // Recover parameter names from workflow input names.
    let param_names = wf.input_names().to_vec();

    Ok(Graph {
        workflow: SendWorkflow(wf),
        param_names,
    })
}
