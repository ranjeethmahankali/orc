use crate::graph::{
    BUILDING_WORKFLOW, IN_WORKFLOW_MODE, WorkflowNode, WorkflowNodeKind, build_nested_workflow_impl,
};
use crate::handle::Handle;
use crate::host::{HANDLE_COUNTER, PLUGIN_SET};
use orc_sdk::{FuncInfo, IH, OH, OrcHandle, OrcHandleBorrowed, Workflow};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use std::sync::atomic::Ordering;

#[pyclass(name = "OrcFunc")]
pub(crate) struct OrcFunc {
    pub(crate) info: FuncInfo,
}

// FuncInfo fields are all Send (String, Option<usize>, function pointer).
unsafe impl Send for OrcFunc {}

#[pymethods]
impl OrcFunc {
    #[pyo3(signature = (*args, n_out=None))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        n_out: Option<usize>,
    ) -> PyResult<PyObject> {
        let n_out = self.resolve_n_out(n_out)?;
        if IN_WORKFLOW_MODE.load(Ordering::Relaxed) {
            self.record_workflow_op(py, args, n_out)
        } else {
            self.immediate_call(py, args, n_out)
        }
    }

    fn __repr__(&self) -> String {
        format!("OrcFunc({:?})", self.info.name)
    }
}

impl OrcFunc {
    /// Resolve the effective output count from the optional user-supplied `n_out`
    /// and the function's declared `n_outputs`. Errors if they conflict.
    fn resolve_n_out(&self, n_out: Option<usize>) -> PyResult<usize> {
        match (self.info.n_outputs, n_out) {
            (Some(declared), Some(requested)) if declared != requested => {
                Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "The function '{}' produces {} output(s), but n_out={} was requested.",
                    self.info.name, declared, requested
                )))
            }
            (_, Some(requested)) => Ok(requested),
            (Some(declared), None) => Ok(declared),
            (None, None) => Ok(1),
        }
    }

    fn immediate_call<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        n_out: usize,
    ) -> PyResult<PyObject> {
        // Validate input count.
        let n_args = args.len();
        if let Some(expected) = self.info.n_inputs
            && n_args != expected
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "The function '{}' expects {} arguments, got {}.",
                self.info.name, expected, n_args
            )));
        }
        let func = self
            .info
            .func
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Null function pointer"))?;
        // Borrow input handles into a contiguous array.
        let input_refs: Vec<PyRef<'_, Handle>> = (0..n_args)
            .map(|i| args.get_item(i)?.extract())
            .collect::<PyResult<_>>()?;
        let raw_inputs: Vec<OrcHandleBorrowed<'_>> =
            input_refs.iter().map(|h| h.inner.borrowed()).collect();
        // Allocate output handles - here we're claiming all the outputs we need with a single fetch_add.
        let base_id = HANDLE_COUNTER.fetch_add(n_out as u64, Ordering::Relaxed);
        let mut raw_outputs: Vec<OrcHandle> = (0..n_out)
            .map(|i| OrcHandle {
                handle: base_id + i as u64,
                ..Default::default()
            })
            .collect();
        let err = py.allow_threads(|| unsafe {
            func(
                0,
                raw_inputs.as_ptr().cast(),
                raw_inputs.len() as u64,
                raw_outputs.as_mut_ptr().cast(),
                n_out as u64,
            )
        });
        orc_sdk::Error::from_raw(err).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Plugin function '{}' failed with error: {}.",
                self.info.name, e
            ))
        })?;
        // Wrap outputs.
        if let Some(last) = raw_outputs.pop() {
            if raw_outputs.is_empty() {
                return Ok(Py::new(py, Handle::new(last))?.into_any());
            } else {
                raw_outputs.push(last);
            }
        }
        Ok(PyList::new(
            py,
            raw_outputs
                .into_iter()
                .map(|h| Py::new(py, Handle::new(h)))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any()
        .unbind())
    }

    fn record_workflow_op<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        n_out: usize,
    ) -> PyResult<PyObject> {
        // Validate input count.
        let n_args = args.len();
        if let Some(expected) = self.info.n_inputs
            && n_args != expected
        {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "The function '{}' expects {} arguments, got {}.",
                self.info.name, expected, n_args
            )));
        }
        // Extract WorkflowNode arguments — each is either an Output(OH) or an Input(idx).
        let arg_kinds: Vec<WorkflowNodeKind> = (0..n_args)
            .map(|i| {
                args.get_item(i)?
                    .extract()
                    .map(|node: PyRef<'_, WorkflowNode>| node.kind)
            })
            .collect::<PyResult<_>>()?;
        // Lock the building workflow, add the function node, and record connections.
        let mut guard = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
        let state = guard
            .last_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?;
        let wf: &mut Workflow = state
            .workflow
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?;
        let mut ihs = vec![IH::default(); n_args];
        let mut ohs = vec![OH::default(); n_out];
        wf.add_function(self.info.clone(), &mut ihs, &mut ohs)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        // Connect upstream outputs or record workflow inputs.
        for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
            if let WorkflowNodeKind::UpstreamNode(oh) = kind {
                wf.connect(*oh, *ih)
                    .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
            }
        }
        for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
            if let WorkflowNodeKind::WorkflowInput(param_idx) = kind {
                state.inputs.push((*ih, *param_idx));
            }
        }
        // Wrap output OHs as WorkflowNodes.
        if n_out == 1 {
            Ok(Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::UpstreamNode(ohs[0]),
                },
            )?
            .into_any())
        } else {
            Ok(PyList::new(
                py,
                ohs.into_iter()
                    .map(|oh| {
                        Py::new(
                            py,
                            WorkflowNode {
                                kind: WorkflowNodeKind::UpstreamNode(oh),
                            },
                        )
                    })
                    .collect::<PyResult<Vec<_>>>()?,
            )?
            .into_any()
            .unbind())
        }
    }
}

// =====================================================================
// WorkflowFunc — @workflow_function decorator
// =====================================================================

#[pyclass(name = "WorkflowFunc")]
pub(crate) struct PyWorkflowFunc {
    pub(crate) func: PyObject,
    pub(crate) name: String,
    pub(crate) param_names: Vec<String>,
    /// Identity key: Python function object pointer, used to deduplicate nested workflow builds.
    pub(crate) func_ptr: usize,
}

// PyObject is not Send by default; all access happens under the GIL.
unsafe impl Send for PyWorkflowFunc {}

#[pymethods]
impl PyWorkflowFunc {
    #[pyo3(signature = (*args))]
    fn __call__<'py>(&self, py: Python<'py>, args: &Bound<'py, PyTuple>) -> PyResult<PyObject> {
        if IN_WORKFLOW_MODE.load(Ordering::Relaxed) {
            self.record_nested_call(py, args)
        } else {
            self.func.bind(py).call1(args)?.extract()
        }
    }

    fn __repr__(&self) -> String {
        format!("WorkflowFunc({:?})", self.name)
    }
}

impl PyWorkflowFunc {
    fn record_nested_call<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<PyObject> {
        let n_args = args.len();
        if n_args != self.param_names.len() {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "WorkflowFunc '{}' expects {} arguments, got {}.",
                self.name,
                self.param_names.len(),
                n_args
            )));
        }

        // Check cache; release lock before any Python call.
        let cached: Option<(String, usize)> = {
            let stack = BUILDING_WORKFLOW
                .lock()
                .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
            stack
                .last()
                .and_then(|frame| frame.nested_built.get(&self.func_ptr))
                .cloned()
        };

        // Extract WorkflowNode kinds from args before taking any lock.
        let arg_kinds: Vec<WorkflowNodeKind> = (0..n_args)
            .map(|i| {
                args.get_item(i)?
                    .extract()
                    .map(|node: PyRef<'_, WorkflowNode>| node.kind)
            })
            .collect::<PyResult<_>>()?;

        // Resolve (name, n_outputs): build and register if this is the first call.
        // No lock is held during the build — it may recurse into BUILDING_WORKFLOW itself.
        let (name, n_outputs): (String, usize) = if let Some(c) = cached {
            c
        } else {
            let (wf, n_out) =
                build_nested_workflow_impl(py, self.func.bind(py), &self.param_names)?;

            let mut stack = BUILDING_WORKFLOW
                .lock()
                .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
            let ps = PLUGIN_SET.lock().map_err(|_| {
                pyo3::exceptions::PyRuntimeError::new_err("Plugin set lock poisoned")
            })?;
            let frame = stack
                .last_mut()
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow frame"))?;
            let outer_wf: &mut Workflow = frame.workflow.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("No workflow being built")
            })?;
            outer_wf
                .push_nested_workflow(self.name.clone(), wf.0, &ps)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
            frame
                .nested_built
                .insert(self.func_ptr, (self.name.clone(), n_out));
            (self.name.clone(), n_out)
        };

        // Add the nested workflow call node and wire it up (pure Rust, single lock).
        let ohs: Vec<OH> = {
            let mut stack = BUILDING_WORKFLOW
                .lock()
                .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
            let frame = stack
                .last_mut()
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow frame"))?;
            let wf: &mut Workflow = frame.workflow.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("No workflow being built")
            })?;

            let mut ihs = vec![IH::default(); n_args];
            let mut ohs = vec![OH::default(); n_outputs];
            wf.add_nested_workflow_call(&name, &mut ihs, &mut ohs)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;

            // Loop 1: connect upstream outputs (borrows wf).
            for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
                if let WorkflowNodeKind::UpstreamNode(oh) = kind {
                    wf.connect(*oh, *ih)
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
                }
            }
            // Loop 2: record workflow inputs (wf no longer used, borrows frame.inputs).
            for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
                if let WorkflowNodeKind::WorkflowInput(param_idx) = kind {
                    frame.inputs.push((*ih, *param_idx));
                }
            }
            ohs
        };

        // Wrap outputs as WorkflowNodes (Python, no lock needed).
        if n_outputs == 1 {
            Ok(Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::UpstreamNode(ohs[0]),
                },
            )?
            .into_any())
        } else {
            Ok(PyList::new(
                py,
                ohs.into_iter()
                    .map(|oh| {
                        Py::new(
                            py,
                            WorkflowNode {
                                kind: WorkflowNodeKind::UpstreamNode(oh),
                            },
                        )
                    })
                    .collect::<PyResult<Vec<_>>>()?,
            )?
            .into_any()
            .unbind())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orc_sdk::FuncInfo;

    fn make_func(n_outputs: Option<usize>) -> OrcFunc {
        OrcFunc {
            info: FuncInfo {
                name: "test_fn".to_string(),
                n_outputs,
                ..Default::default()
            },
        }
    }

    #[test]
    fn t_resolve_n_out_defaults_to_one() {
        assert_eq!(make_func(None).resolve_n_out(None).unwrap(), 1);
    }

    #[test]
    fn t_resolve_n_out_uses_declared_when_no_request() {
        assert_eq!(make_func(Some(3)).resolve_n_out(None).unwrap(), 3);
    }

    #[test]
    fn t_resolve_n_out_uses_requested_when_no_declaration() {
        assert_eq!(make_func(None).resolve_n_out(Some(5)).unwrap(), 5);
    }

    #[test]
    fn t_resolve_n_out_matching_declared_and_requested() {
        assert_eq!(make_func(Some(2)).resolve_n_out(Some(2)).unwrap(), 2);
    }

    #[test]
    fn t_resolve_n_out_conflict_returns_err() {
        assert!(make_func(Some(2)).resolve_n_out(Some(3)).is_err());
    }
}

/// Decorator that wraps a Python function as a `WorkflowFunc`.
/// When called inside `make_workflow`, records a nested workflow call node instead of
/// executing immediately. When called outside, behaves like the original function.
#[pyfunction]
pub(crate) fn workflow_function(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let inspect = py.import("inspect")?;
    let sig = inspect.call_method1("signature", (func,))?;
    let params = sig.getattr("parameters")?;
    let param_names: Vec<String> = params
        .call_method0("keys")?
        .try_iter()?
        .map(|k| k.and_then(|k| k.extract::<String>()))
        .collect::<PyResult<_>>()?;
    let name: String = func.getattr("__name__")?.extract()?;
    let func_ptr = func.as_ptr() as usize;
    Ok(Py::new(
        py,
        PyWorkflowFunc {
            func: func.clone().into(),
            name,
            param_names,
            func_ptr,
        },
    )?
    .into_any())
}
