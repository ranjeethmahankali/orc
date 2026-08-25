use crate::graph::{
    BUILDING_WORKFLOW, IN_WORKFLOW_MODE, WORKFLOW_INPUTS, WorkflowNode, WorkflowNodeKind,
};
use crate::handle::Handle;
use crate::host::HANDLE_COUNTER;
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
        if n_out == 1 {
            Ok(Py::new(py, Handle::new(raw_outputs.pop().unwrap()))?.into_any())
        } else {
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
        // Lock the building workflow and add the function node.
        let mut wf_guard = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
        let wf: &mut Workflow = wf_guard
            .as_mut()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?;
        let mut ihs = vec![IH::default(); n_args];
        let mut ohs = vec![OH::default(); n_out];
        wf.add_function(self.info.clone(), &mut ihs, &mut ohs)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
        // Connect outputs or record workflow inputs.
        let mut wi_guard = WORKFLOW_INPUTS
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
        for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
            match kind {
                WorkflowNodeKind::UpstreamNode(oh) => {
                    wf.connect(*oh, *ih)
                        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
                }
                WorkflowNodeKind::WorkflowInput(param_idx) => {
                    wi_guard.push((*ih, *param_idx));
                }
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
