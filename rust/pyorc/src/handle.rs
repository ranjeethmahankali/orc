use orc_sdk::{
    OrcHandle, ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64,
    ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Wrapper to make OrcHandle Send (required by #[pyclass]).
/// SAFETY: OrcHandle contains raw pointers to plugin-managed memory. This memory is
/// thread-safe (plugins handle their own synchronization). The free_fn is a function
/// pointer to a thread-safe deallocation routine.
pub(crate) struct SendOrcHandle(pub OrcHandle);
unsafe impl Send for SendOrcHandle {}
unsafe impl Sync for SendOrcHandle {}

#[pyclass(name = "Handle")]
pub(crate) struct Handle {
    pub(crate) inner: SendOrcHandle,
}

impl Handle {
    pub fn new(handle: OrcHandle) -> Self {
        Handle {
            inner: SendOrcHandle(handle),
        }
    }
}

#[pymethods]
impl Handle {
    #[getter]
    fn type_id(&self) -> u64 {
        self.inner.0.type_id
    }

    #[getter]
    fn n_items(&self) -> u64 {
        self.inner.0.n_items
    }

    #[getter]
    fn item_size(&self) -> u64 {
        self.inner.0.item_size
    }

    #[getter]
    fn dims(&self) -> Vec<i32> {
        self.inner.0.dims.to_vec()
    }

    #[getter]
    fn __array_interface__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let h = &self.inner.0;
        let typestr = type_id_to_typestr(h.type_id)?;
        let dict = PyDict::new(py);
        dict.set_item("version", 3)?;
        dict.set_item("shape", (h.n_items,))?;
        dict.set_item("typestr", typestr)?;
        dict.set_item("data", (h.items as usize, false))?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!(
            "<Handle type_id={:#x} n_items={}>",
            self.inner.0.type_id, self.inner.0.n_items
        )
    }
}

fn type_id_to_typestr(type_id: u64) -> PyResult<&'static str> {
    match type_id {
        ORC_TYPE_U8 => Ok("|u1"),
        ORC_TYPE_U16 => Ok("<u2"),
        ORC_TYPE_U32 => Ok("<u4"),
        ORC_TYPE_U64 => Ok("<u8"),
        ORC_TYPE_I8 => Ok("|i1"),
        ORC_TYPE_I16 => Ok("<i2"),
        ORC_TYPE_I32 => Ok("<i4"),
        ORC_TYPE_I64 => Ok("<i8"),
        ORC_TYPE_F32 => Ok("<f4"),
        ORC_TYPE_F64 => Ok("<f8"),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported type_id for numpy: {:#x}",
            type_id
        ))),
    }
}
