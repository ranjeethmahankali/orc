use crate::graph::{GraphNode, GraphNodeKind, BUILDING_WORKFLOW, IN_GRAPH_MODE, WORKFLOW_INPUTS};
use crate::handle::Handle;
use crate::host::HANDLE_COUNTER;
use orc_sdk::{FuncInfo, IH, OH, OrcHandle};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use std::mem::ManuallyDrop;
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
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<PyObject> {
        if IN_GRAPH_MODE.load(Ordering::Relaxed) {
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
        let n_args = args.len();
        if let Some(expected) = self.info.n_inputs {
            if n_args != expected {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "The function '{}' expects {} arguments, got {}.",
                    self.info.name, expected, n_args
                )));
            }
        }

        let func = self.info.func.ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("null function pointer")
        })?;
        let n_out = self.info.n_outputs.unwrap_or(1);

        // Build contiguous input array (borrowed copies, free_fn = None).
        let mut raw_inputs: Vec<ManuallyDrop<OrcHandle>> = Vec::with_capacity(n_args);
        for i in 0..n_args {
            let h: PyRef<'_, Handle> = args.get_item(i)?.extract()?;
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

        let base_id = HANDLE_COUNTER.fetch_add(n_out as u64, Ordering::Relaxed);
        let mut raw_outputs: Vec<ManuallyDrop<OrcHandle>> = (0..n_out)
            .map(|i| {
                ManuallyDrop::new(OrcHandle {
                    handle: base_id + i as u64,
                    ..Default::default()
                })
            })
            .collect();

        // Cast pointers to usize so the closure is Ungil (raw ptrs are !Sync).
        let in_addr = raw_inputs.as_ptr() as usize;
        let n_in = raw_inputs.len() as u64;
        let out_addr = raw_outputs.as_mut_ptr() as usize;
        let n_out_u64 = n_out as u64;

        // SAFETY: Plugin function is pure native code — no Python interaction.
        py.allow_threads(|| unsafe {
            func(
                0,
                in_addr as *const OrcHandle,
                n_in,
                out_addr as *mut OrcHandle,
                n_out_u64,
            );
        });

        if n_out == 1 {
            let h = ManuallyDrop::into_inner(raw_outputs.pop().unwrap());
            Ok(Py::new(py, Handle::new(h))?.into_any())
        } else {
            let list = PyList::empty(py);
            for mh in raw_outputs {
                let h = ManuallyDrop::into_inner(mh);
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
        let n_args = args.len();
        let n_out = self.info.n_outputs.unwrap_or(1);

        // Extract GraphNode arguments — each is either an Output(OH) or an Input(idx).
        let arg_kinds: Vec<GraphNodeKind> = (0..n_args)
            .map(|i| {
                let node: PyRef<'_, GraphNode> = args.get_item(i)?.extract()?;
                Ok(node.kind)
            })
            .collect::<PyResult<_>>()?;

        // Lock the building workflow and add the function node.
        let mut wf_guard = BUILDING_WORKFLOW
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("workflow lock poisoned"))?;
        let wf = &mut wf_guard.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("no workflow being built")
        })?.0;

        let mut ihs = vec![IH::default(); n_args];
        let mut ohs = vec![OH::default(); n_out];
        wf.add_function(self.info.clone(), &mut ihs, &mut ohs)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;

        // Connect or record workflow inputs.
        let mut wi_guard = WORKFLOW_INPUTS
            .lock()
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("lock poisoned"))?;
        for (kind, ih) in arg_kinds.iter().zip(ihs.iter()) {
            match kind {
                GraphNodeKind::Output(oh) => {
                    wf.connect(*oh, *ih).map_err(|e| {
                        pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
                    })?;
                }
                GraphNodeKind::Input(param_idx) => {
                    // Leave this IH unconnected — record it as a workflow input.
                    wi_guard.push((*ih, *param_idx));
                }
            }
        }

        if n_out == 1 {
            Ok(Py::new(py, GraphNode { kind: GraphNodeKind::Output(ohs[0]) })?.into_any())
        } else {
            let list = PyList::empty(py);
            for oh in ohs {
                list.append(Py::new(py, GraphNode { kind: GraphNodeKind::Output(oh) })?)?;
            }
            Ok(list.into_any().unbind())
        }
    }
}
