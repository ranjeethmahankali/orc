use orc_sdk::{
    ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_U8,
    ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64, OrcHandle,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::ops::Deref;

/// Wrapper to make OrcHandle Send+Sync (required by #[pyclass]).
/// SAFETY: OrcHandle contains raw pointers to plugin-managed memory. This memory is
/// thread-safe (plugins handle their own synchronization). The free_fn is a function
/// pointer to a thread-safe deallocation routine.
#[repr(transparent)]
pub(crate) struct SendOrcHandle(pub OrcHandle);
unsafe impl Send for SendOrcHandle {}
unsafe impl Sync for SendOrcHandle {}

impl Deref for SendOrcHandle {
    type Target = OrcHandle;
    fn deref(&self) -> &OrcHandle {
        &self.0
    }
}

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
        self.inner.type_id
    }

    #[getter]
    fn n_items(&self) -> u64 {
        self.inner.n_items
    }

    #[getter]
    fn item_size(&self) -> u64 {
        self.inner.item_size
    }

    #[getter]
    fn dims(&self) -> (i32, i32, i32, i32, i32, i32, i32) {
        let d = self.inner.dims;
        (d[0], d[1], d[2], d[3], d[4], d[5], d[6])
    }

    #[getter]
    fn __array_interface__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let h = &*self.inner;
        let (typestr, scalar_size) = type_id_to_typestr(h.type_id)?;
        // item_size can be a multiple of the scalar size rather than exactly equal to it -- an
        // aggregate (e.g. an F64x3 handle) shares its scalar's type_id, distinguished only by a
        // larger item_size. Report that as an extra trailing shape dimension (numpy's convention
        // for arrays of fixed-size vectors) instead of reinterpreting the buffer as a flat array
        // of scalars, which would silently fold every component into the item axis.
        let item_size = h.item_size as usize;
        if item_size == 0 || item_size % scalar_size != 0 {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "handle item_size {item_size} is not a multiple of the {scalar_size}-byte scalar \
                 size for type_id {:#x}",
                h.type_id
            )));
        }
        let n_components = item_size / scalar_size;
        let dict = PyDict::new(py);
        dict.set_item("version", 3)?;
        if n_components == 1 {
            dict.set_item("shape", (h.n_items,))?;
        } else {
            dict.set_item("shape", (h.n_items, n_components as u64))?;
        }
        dict.set_item("typestr", typestr)?;
        dict.set_item("data", (h.items as usize, false))?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!(
            "<Handle type_id={:#x} n_items={}>",
            self.inner.type_id, self.inner.n_items
        )
    }
}

/// Returns the Python array interface typestring and scalar byte size for a given type ID.
///
/// Typestring format: `<endian><kind><bytes>` where endian is `|` (not applicable, single-byte)
/// or `<` (little-endian, multi-byte), kind is `u`/`i`/`f`, and bytes is the byte count -- which
/// is also the scalar size returned alongside it, used by callers to detect aggregates (handles
/// whose item_size is a multiple of, rather than equal to, this scalar size).
fn type_id_to_typestr(type_id: u64) -> PyResult<(&'static str, usize)> {
    match type_id {
        ORC_TYPE_U8 => Ok(("|u1", 1)),
        ORC_TYPE_U16 => Ok(("<u2", 2)),
        ORC_TYPE_U32 => Ok(("<u4", 4)),
        ORC_TYPE_U64 => Ok(("<u8", 8)),
        ORC_TYPE_I8 => Ok(("|i1", 1)),
        ORC_TYPE_I16 => Ok(("<i2", 2)),
        ORC_TYPE_I32 => Ok(("<i4", 4)),
        ORC_TYPE_I64 => Ok(("<i8", 8)),
        ORC_TYPE_F32 => Ok(("<f4", 4)),
        ORC_TYPE_F64 => Ok(("<f8", 8)),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unsupported type_id for numpy: {:#x}",
            type_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_type_id_to_typestr_all_known_types() {
        assert_eq!(type_id_to_typestr(ORC_TYPE_U8).unwrap(), ("|u1", 1));
        assert_eq!(type_id_to_typestr(ORC_TYPE_U16).unwrap(), ("<u2", 2));
        assert_eq!(type_id_to_typestr(ORC_TYPE_U32).unwrap(), ("<u4", 4));
        assert_eq!(type_id_to_typestr(ORC_TYPE_U64).unwrap(), ("<u8", 8));
        assert_eq!(type_id_to_typestr(ORC_TYPE_I8).unwrap(), ("|i1", 1));
        assert_eq!(type_id_to_typestr(ORC_TYPE_I16).unwrap(), ("<i2", 2));
        assert_eq!(type_id_to_typestr(ORC_TYPE_I32).unwrap(), ("<i4", 4));
        assert_eq!(type_id_to_typestr(ORC_TYPE_I64).unwrap(), ("<i8", 8));
        assert_eq!(type_id_to_typestr(ORC_TYPE_F32).unwrap(), ("<f4", 4));
        assert_eq!(type_id_to_typestr(ORC_TYPE_F64).unwrap(), ("<f8", 8));
    }

    #[test]
    fn t_type_id_to_typestr_unknown_returns_err() {
        assert!(type_id_to_typestr(0xDEAD_BEEF).is_err());
    }
}
