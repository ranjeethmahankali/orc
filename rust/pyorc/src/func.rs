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
    #[pyo3(signature = (*args))]
    fn __call__<'py>(&self, py: Python<'py>, args: &Bound<'py, PyTuple>) -> PyResult<PyObject> {
        if IN_WORKFLOW_MODE.load(Ordering::Relaxed) {
            self.deferred_call(py, args)
        } else {
            self.immediate_call(py, args)
        }
    }

    fn __repr__(&self) -> String {
        format!("OrcFunc({:?})", self.info.name)
    }
}

impl OrcFunc {
    fn immediate_call<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
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
        let n_out = self.info.n_outputs.unwrap_or(1);
        // Borrow input handles into a contiguous array.
        let input_refs: Vec<PyRef<'_, Handle>> = (0..n_args)
            .map(|i| args.get_item(i)?.extract())
            .collect::<PyResult<_>>()?;
        let raw_inputs: Vec<OrcHandleBorrowed<'_>> =
            input_refs.iter().map(|h| h.inner.borrowed()).collect();
        // Allocate output handles.
        let base_id = HANDLE_COUNTER.fetch_add(n_out as u64, Ordering::Relaxed);
        let mut raw_outputs: Vec<OrcHandle> = (0..n_out)
            .map(|i| OrcHandle {
                handle: base_id + i as u64,
                ..Default::default()
            })
            .collect();
        // Call the plugin function with the GIL released.
        // Pointers are cast to usize so the closure satisfies Ungil (raw ptrs are !Sync).
        let in_addr = raw_inputs.as_ptr() as usize;
        let n_in = raw_inputs.len() as u64;
        let out_addr = raw_outputs.as_mut_ptr() as usize;
        let n_out_u64 = n_out as u64;
        let err = py.allow_threads(|| unsafe {
            func(
                0,
                in_addr as *const OrcHandle,
                n_in,
                out_addr as *mut OrcHandle,
                n_out_u64,
            )
        });
        if err != orc_sdk::ORC_ERROR_NONE {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Plugin function '{}' failed with error code {:#x}.",
                self.info.name, err
            )));
        }
        // Wrap outputs.
        if n_out == 1 {
            Ok(Py::new(py, Handle::new(raw_outputs.pop().unwrap()))?.into_any())
        } else {
            let list = PyList::empty(py);
            for h in raw_outputs {
                list.append(Py::new(py, Handle::new(h))?)?;
            }
            Ok(list.into_any().unbind())
        }
    }

    fn deferred_call<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
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
        let n_out = self.info.n_outputs.unwrap_or(1);
        // Extract WorkflowNode arguments — each is either an Output(OH) or an Input(idx).
        let arg_kinds: Vec<WorkflowNodeKind> = (0..n_args)
            .map(|i| {
                let node: PyRef<'_, WorkflowNode> = args.get_item(i)?.extract()?;
                Ok(node.kind)
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
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;
        // Connect outputs or record workflow inputs.
        let mut wi_guard = WORKFLOW_INPUTS
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock poisoned"))?;
        for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
            match kind {
                WorkflowNodeKind::Output(oh) => {
                    wf.connect(*oh, *ih).map_err(|e| {
                        pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
                    })?;
                }
                WorkflowNodeKind::Input(param_idx) => {
                    wi_guard.push((*ih, *param_idx));
                }
            }
        }
        // Wrap output OHs as WorkflowNodes.
        if n_out == 1 {
            Ok(Py::new(
                py,
                WorkflowNode {
                    kind: WorkflowNodeKind::Output(ohs[0]),
                },
            )?
            .into_any())
        } else {
            let list = PyList::empty(py);
            for oh in ohs {
                list.append(Py::new(
                    py,
                    WorkflowNode {
                        kind: WorkflowNodeKind::Output(oh),
                    },
                )?)?;
            }
            Ok(list.into_any().unbind())
        }
    }
}
