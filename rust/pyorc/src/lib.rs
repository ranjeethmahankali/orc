mod func;
mod graph;
mod handle;
mod host;
mod stubs;

use func::{OrcFunc, PyWorkflowFunc, workflow_function};
use graph::{BUILDING_WORKFLOW, IN_WORKFLOW_MODE, PyWorkflow, WorkflowNode, WorkflowNodeKind};
use handle::Handle;
use host::{HANDLE_COUNTER, PLUGIN_SET, REGISTRY};
use orc_sdk::{
    Deck, ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32, ORC_TYPE_I64,
    ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64, OrcHandle, OrcMark, Workflow,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyTuple};
use std::sync::atomic::Ordering;

#[pymodule(name = "orc")]
fn pyorc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Handle>()?;
    m.add_class::<WorkflowNode>()?;
    m.add_class::<PyWorkflow>()?;
    m.add_class::<PyWorkflowFunc>()?;
    m.add_function(wrap_pyfunction!(load_plugins, m)?)?;
    m.add_function(wrap_pyfunction!(workflow_function, m)?)?;
    m.add_function(wrap_pyfunction!(make_deck, m)?)?;
    m.add_function(wrap_pyfunction!(read_deck, m)?)?;
    m.add_function(wrap_pyfunction!(make_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(run_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(save_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(load_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(deck_to_str, m)?)?;
    // Builtin type ID constants — mirrors the ORC_TYPE_* values from orc_abi.h.
    m.add("ORC_TYPE_U8", ORC_TYPE_U8)?;
    m.add("ORC_TYPE_U16", ORC_TYPE_U16)?;
    m.add("ORC_TYPE_U32", ORC_TYPE_U32)?;
    m.add("ORC_TYPE_U64", ORC_TYPE_U64)?;
    m.add("ORC_TYPE_I8", ORC_TYPE_I8)?;
    m.add("ORC_TYPE_I16", ORC_TYPE_I16)?;
    m.add("ORC_TYPE_I32", ORC_TYPE_I32)?;
    m.add("ORC_TYPE_I64", ORC_TYPE_I64)?;
    m.add("ORC_TYPE_F32", ORC_TYPE_F32)?;
    m.add("ORC_TYPE_F64", ORC_TYPE_F64)?;
    Ok(())
}

// =====================================================================
// Module-level functions
// =====================================================================

#[pyfunction]
fn load_plugins(py: Python<'_>, search_dir: &str) -> PyResult<()> {
    let mut ps = PLUGIN_SET
        .lock()
        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Lock error."))?;
    // Record which plugins existed before so we only register the new ones.
    let prev_count = ps.plugins().len();
    ps.append_from_dir(std::path::Path::new(search_dir), &host::HOST)
        .map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to load plugins: {}.", e))
        })?;
    // Register newly loaded plugin functions as module attributes.
    let module = PyModule::import(py, "orc")?;
    for func_info in ps.plugins()[prev_count..]
        .iter()
        .flat_map(|plugin| plugin.functions().iter())
    {
        module.setattr(
            pyo3::types::PyString::new(py, &func_info.name),
            Py::new(
                py,
                OrcFunc {
                    info: func_info.clone(),
                },
            )?,
        )?;
    }
    stubs::generate_stubs(py, &ps)
}

#[pyfunction]
#[pyo3(signature = (data, dtype=None))]
fn make_deck(py: Python<'_>, data: &Bound<'_, PyAny>, dtype: Option<&str>) -> PyResult<PyObject> {
    let type_id = dtype.map(parse_dtype).transpose()?;
    if IN_WORKFLOW_MODE.load(Ordering::Acquire) {
        make_deck_deferred(py, data, type_id)
    } else {
        let handle = create_orc_handle(py, data, type_id)?;
        Ok(Py::new(py, Handle::new(handle))?.into_any())
    }
}

fn parse_dtype(s: &str) -> PyResult<u64> {
    match s {
        "u8" => Ok(ORC_TYPE_U8),
        "u16" => Ok(ORC_TYPE_U16),
        "u32" => Ok(ORC_TYPE_U32),
        "u64" => Ok(ORC_TYPE_U64),
        "i8" => Ok(ORC_TYPE_I8),
        "i16" => Ok(ORC_TYPE_I16),
        "i32" => Ok(ORC_TYPE_I32),
        "i64" => Ok(ORC_TYPE_I64),
        "f32" => Ok(ORC_TYPE_F32),
        "f64" => Ok(ORC_TYPE_F64),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unknown dtype: '{}'.",
            s
        ))),
    }
}

/// Converts a single deck item into a Python object. Scalars become plain Python numbers;
/// aggregates (`[T; N]`, item_size a multiple of the scalar size) become Python tuples of their
/// N components. Mirrors `orc_sdk::DeckItemDisplay`'s approach: concrete impls per scalar type
/// (never a blanket `impl<T: IntoPyObject> ToPyDeckItem for T`) so the `[T; N]` impl doesn't
/// overlap with them.
trait ToPyDeckItem {
    fn to_py_item(&self, py: Python<'_>) -> PyResult<PyObject>;
}

macro_rules! impl_to_py_deck_item_scalar {
    ($($t:ty),* $(,)?) => {
        $(
            impl ToPyDeckItem for $t {
                fn to_py_item(&self, py: Python<'_>) -> PyResult<PyObject> {
                    Ok(self.into_pyobject(py)?.into_any().unbind())
                }
            }
        )*
    };
}
impl_to_py_deck_item_scalar!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

impl<T: ToPyDeckItem, const N: usize> ToPyDeckItem for [T; N] {
    fn to_py_item(&self, py: Python<'_>) -> PyResult<PyObject> {
        let parts: PyResult<Vec<PyObject>> = self.iter().map(|v| v.to_py_item(py)).collect();
        Ok(PyTuple::new(py, parts?)?.into_any().unbind())
    }
}

fn read_items_as_pylist<U: orc_sdk::TOrcData + ToPyDeckItem>(
    py: Python<'_>,
    handle: &OrcHandle,
) -> PyResult<PyObject> {
    let items: &[U] = handle
        .items::<U>()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    let py_items: Vec<PyObject> = items
        .iter()
        .map(|v| v.to_py_item(py))
        .collect::<PyResult<_>>()?;
    deck_to_nested_py_list(py, py_items, handle.marks())
}

/// Dispatches to `read_items_as_pylist::<T>` for a plain scalar (n_components == 1), or
/// `read_items_as_pylist::<[T; N]>` for an aggregate, covering N = 2..=32.
fn read_typed<T: orc_sdk::TOrcData + ToPyDeckItem>(
    py: Python<'_>,
    handle: &OrcHandle,
) -> PyResult<PyObject> {
    let scalar_size = size_of::<T>();
    if handle.item_size == 0 || !(handle.item_size as usize).is_multiple_of(scalar_size) {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "item_size {} is not a valid multiple of the scalar size {scalar_size}",
            handle.item_size
        )));
    }
    match (handle.item_size as usize) / scalar_size {
        1 => read_items_as_pylist::<T>(py, handle),
        2 => read_items_as_pylist::<[T; 2]>(py, handle),
        3 => read_items_as_pylist::<[T; 3]>(py, handle),
        4 => read_items_as_pylist::<[T; 4]>(py, handle),
        5 => read_items_as_pylist::<[T; 5]>(py, handle),
        6 => read_items_as_pylist::<[T; 6]>(py, handle),
        7 => read_items_as_pylist::<[T; 7]>(py, handle),
        8 => read_items_as_pylist::<[T; 8]>(py, handle),
        9 => read_items_as_pylist::<[T; 9]>(py, handle),
        10 => read_items_as_pylist::<[T; 10]>(py, handle),
        11 => read_items_as_pylist::<[T; 11]>(py, handle),
        12 => read_items_as_pylist::<[T; 12]>(py, handle),
        13 => read_items_as_pylist::<[T; 13]>(py, handle),
        14 => read_items_as_pylist::<[T; 14]>(py, handle),
        15 => read_items_as_pylist::<[T; 15]>(py, handle),
        16 => read_items_as_pylist::<[T; 16]>(py, handle),
        17 => read_items_as_pylist::<[T; 17]>(py, handle),
        18 => read_items_as_pylist::<[T; 18]>(py, handle),
        19 => read_items_as_pylist::<[T; 19]>(py, handle),
        20 => read_items_as_pylist::<[T; 20]>(py, handle),
        21 => read_items_as_pylist::<[T; 21]>(py, handle),
        22 => read_items_as_pylist::<[T; 22]>(py, handle),
        23 => read_items_as_pylist::<[T; 23]>(py, handle),
        24 => read_items_as_pylist::<[T; 24]>(py, handle),
        25 => read_items_as_pylist::<[T; 25]>(py, handle),
        26 => read_items_as_pylist::<[T; 26]>(py, handle),
        27 => read_items_as_pylist::<[T; 27]>(py, handle),
        28 => read_items_as_pylist::<[T; 28]>(py, handle),
        29 => read_items_as_pylist::<[T; 29]>(py, handle),
        30 => read_items_as_pylist::<[T; 30]>(py, handle),
        31 => read_items_as_pylist::<[T; 31]>(py, handle),
        32 => read_items_as_pylist::<[T; 32]>(py, handle),
        n => Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "unsupported aggregate size {n} (item_size={}, scalar_size={scalar_size})",
            handle.item_size
        ))),
    }
}

#[pyfunction]
fn read_deck(py: Python<'_>, handle: &Handle) -> PyResult<PyObject> {
    let handle = &handle.inner;
    match handle.type_id {
        ORC_TYPE_U8 => read_typed::<u8>(py, handle),
        ORC_TYPE_U16 => read_typed::<u16>(py, handle),
        ORC_TYPE_U32 => read_typed::<u32>(py, handle),
        ORC_TYPE_U64 => read_typed::<u64>(py, handle),
        ORC_TYPE_I8 => read_typed::<i8>(py, handle),
        ORC_TYPE_I16 => read_typed::<i16>(py, handle),
        ORC_TYPE_I32 => read_typed::<i32>(py, handle),
        ORC_TYPE_I64 => read_typed::<i64>(py, handle),
        ORC_TYPE_F32 => read_typed::<f32>(py, handle),
        ORC_TYPE_F64 => read_typed::<f64>(py, handle),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unknown type_id: {:#x}",
            handle.type_id
        ))),
    }
}

#[pyfunction]
fn deck_to_str(py: Python<'_>, handle: &Handle) -> PyResult<PyObject> {
    let out = host::host_deck_to_str(&handle.inner)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?;
    Ok(Py::new(py, Handle::new(out))?.into_any())
}

#[pyfunction]
fn make_workflow(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<PyWorkflow> {
    graph::make_workflow_impl(py, func)
}

#[pyfunction]
#[pyo3(signature = (graph, *args, **kwargs))]
fn run_workflow<'py>(
    graph: &PyWorkflow,
    py: Python<'py>,
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<PyObject> {
    graph.run_impl(py, args, kwargs)
}

#[pyfunction]
fn save_workflow(graph: &PyWorkflow, path: &str) -> PyResult<()> {
    graph::save_workflow_impl(graph, path)
}

#[pyfunction]
fn load_workflow(path: &str) -> PyResult<PyWorkflow> {
    graph::load_workflow_impl(path)
}

// =====================================================================
// make_deck internals
// =====================================================================

fn create_orc_handle(
    _py: Python<'_>,
    data: &Bound<'_, PyAny>,
    type_id: Option<u64>,
) -> PyResult<OrcHandle> {
    // Flatten nested lists into a flat run of component values plus per-item nesting depths.
    // Every item contributes exactly `n_components` consecutive entries to `components` --  1 for
    // a plain scalar, N for an aggregate (a tuple leaf) -- so item `i`'s components live at
    // `components[i * n_components .. (i + 1) * n_components]`. `py_to_deck` validates that
    // `n_components` is consistent (and in 1..=32) as it goes, so there's no separate leaf-Vec
    // per item and no post-hoc validation pass over the whole tree.
    let mut components: Vec<Bound<'_, PyAny>> = Vec::new();
    let mut depths: Vec<u8> = Vec::new();
    let mut n_components: Option<usize> = None;
    py_to_deck(data, 0, &mut components, &mut depths, &mut n_components)?;
    let n_components = n_components.unwrap_or(1);

    // Detect or use the provided type, based on the flattened scalar values.
    let type_id = match type_id {
        Some(id) => id,
        None if components.is_empty() => ORC_TYPE_F64,
        None => detect_type(&components)?,
    };

    // Build a typed Deck and allocate in the host registry.
    let mut handle = OrcHandle {
        handle: HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
        ..Default::default()
    };
    match type_id {
        ORC_TYPE_U8 => build_deck::<u8>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_U16 => build_deck::<u16>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_U32 => build_deck::<u32>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_U64 => build_deck::<u64>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_I8 => build_deck::<i8>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_I16 => build_deck::<i16>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_I32 => build_deck::<i32>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_I64 => build_deck::<i64>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_F32 => build_deck::<f32>(n_components, &components, &depths, &mut handle)?,
        ORC_TYPE_F64 => build_deck::<f64>(n_components, &components, &depths, &mut handle)?,
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported type_id: {:#x}",
                type_id
            )));
        }
    }
    Ok(handle)
}

fn build_deck_scalar<'py, T: orc_sdk::TOrcData + pyo3::FromPyObject<'py>>(
    components: &[Bound<'py, PyAny>],
    depths: &[u8],
    handle: &mut OrcHandle,
) -> PyResult<()> {
    let mut deck = Deck::<T>::default();
    for (v, &depth) in components.iter().zip(depths.iter()) {
        deck.push(v.extract::<T>()?, depth);
    }
    REGISTRY
        .alloc_with_value(Some(deck), handle)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
}

fn build_deck_arr<'py, T: orc_sdk::TOrcData + pyo3::FromPyObject<'py>, const N: usize>(
    components: &[Bound<'py, PyAny>],
    depths: &[u8],
    handle: &mut OrcHandle,
) -> PyResult<()>
where
    [T; N]: orc_sdk::TOrcData,
{
    let mut deck = Deck::<[T; N]>::default();
    for (i, &depth) in depths.iter().enumerate() {
        let base = i * N;
        let mut arr: [T; N] = std::array::from_fn(|_| T::default());
        for (j, slot) in arr.iter_mut().enumerate() {
            *slot = components[base + j].extract()?;
        }
        deck.push(arr, depth);
    }
    REGISTRY
        .alloc_with_value(Some(deck), handle)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))
}

/// Dispatches to `build_deck_scalar::<T>` for a plain scalar (n_components == 1), or
/// `build_deck_arr::<T, N>` for an aggregate, covering N = 2..=32.
fn build_deck<'py, T: orc_sdk::TOrcData + pyo3::FromPyObject<'py>>(
    n_components: usize,
    components: &[Bound<'py, PyAny>],
    depths: &[u8],
    handle: &mut OrcHandle,
) -> PyResult<()> {
    match n_components {
        1 => build_deck_scalar::<T>(components, depths, handle),
        2 => build_deck_arr::<T, 2>(components, depths, handle),
        3 => build_deck_arr::<T, 3>(components, depths, handle),
        4 => build_deck_arr::<T, 4>(components, depths, handle),
        5 => build_deck_arr::<T, 5>(components, depths, handle),
        6 => build_deck_arr::<T, 6>(components, depths, handle),
        7 => build_deck_arr::<T, 7>(components, depths, handle),
        8 => build_deck_arr::<T, 8>(components, depths, handle),
        9 => build_deck_arr::<T, 9>(components, depths, handle),
        10 => build_deck_arr::<T, 10>(components, depths, handle),
        11 => build_deck_arr::<T, 11>(components, depths, handle),
        12 => build_deck_arr::<T, 12>(components, depths, handle),
        13 => build_deck_arr::<T, 13>(components, depths, handle),
        14 => build_deck_arr::<T, 14>(components, depths, handle),
        15 => build_deck_arr::<T, 15>(components, depths, handle),
        16 => build_deck_arr::<T, 16>(components, depths, handle),
        17 => build_deck_arr::<T, 17>(components, depths, handle),
        18 => build_deck_arr::<T, 18>(components, depths, handle),
        19 => build_deck_arr::<T, 19>(components, depths, handle),
        20 => build_deck_arr::<T, 20>(components, depths, handle),
        21 => build_deck_arr::<T, 21>(components, depths, handle),
        22 => build_deck_arr::<T, 22>(components, depths, handle),
        23 => build_deck_arr::<T, 23>(components, depths, handle),
        24 => build_deck_arr::<T, 24>(components, depths, handle),
        25 => build_deck_arr::<T, 25>(components, depths, handle),
        26 => build_deck_arr::<T, 26>(components, depths, handle),
        27 => build_deck_arr::<T, 27>(components, depths, handle),
        28 => build_deck_arr::<T, 28>(components, depths, handle),
        29 => build_deck_arr::<T, 29>(components, depths, handle),
        30 => build_deck_arr::<T, 30>(components, depths, handle),
        31 => build_deck_arr::<T, 31>(components, depths, handle),
        32 => build_deck_arr::<T, 32>(components, depths, handle),
        // `n_components` is always produced by `py_to_deck`, which already validates 1..=32.
        _ => unreachable!(),
    }
}

fn make_deck_deferred(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    type_id: Option<u64>,
) -> PyResult<PyObject> {
    let handle = create_orc_handle(py, data, type_id)?;
    // Add as a constant node that owns the handle.
    let mut wf_guard = BUILDING_WORKFLOW
        .lock()
        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("Workflow lock poisoned"))?;
    let wf: &mut Workflow = &mut wf_guard
        .last_mut()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?
        .workflow;
    let oh = wf
        .add_constant(handle)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{}", e)))?
        .1;
    Ok(Py::new(
        py,
        WorkflowNode {
            kind: WorkflowNodeKind::UpstreamNode(oh),
        },
    )?
    .into_any())
}

// =====================================================================
// Bidirectional conversion: Python lists <-> Deck (items + depths)
// =====================================================================

/// Recursively flatten a Python value (scalar, tuple, or nested list) into a flat run of
/// component values (`components`) and per-item nesting depths (`depths`). First element of each
/// list inherits depth + 1; subsequent elements get depth 0 (continuation).
///
/// Lists provide *structural* nesting (marks); tuples do not -- a tuple is a single leaf item
/// whose own elements become that item's aggregate components (mapped to `[T; N]` on the Rust
/// side), not further nested structure. So `[(1, 2), (3, 4)]` is a flat 2-item deck of
/// 2-component aggregates, not a depth-2 list of scalars. Every leaf item (scalar or tuple) must
/// contribute the same number of components -- `n_components` records that count and is
/// validated here as each leaf is pushed, rather than in a separate pass afterwards.
fn py_to_deck<'py>(
    data: &Bound<'py, PyAny>,
    depth: u8,
    components: &mut Vec<Bound<'py, PyAny>>,
    depths: &mut Vec<u8>,
    n_components: &mut Option<usize>,
) -> PyResult<()> {
    if data.is_instance_of::<PyTuple>() {
        let tuple = data.downcast::<PyTuple>()?;
        for elem in tuple.iter() {
            if elem.is_instance_of::<PyTuple>() || elem.is_instance_of::<PyList>() {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Nested tuples or lists are not supported inside an aggregate item (tuple).",
                ));
            }
        }
        push_leaf(
            tuple.iter(),
            tuple.len(),
            depth,
            components,
            depths,
            n_components,
        )
    } else if data.is_instance_of::<PyList>() {
        for (i, elem) in data.try_iter()?.enumerate() {
            py_to_deck(
                &elem?,
                if i == 0 { depth + 1 } else { 0 },
                components,
                depths,
                n_components,
            )?;
        }
        Ok(())
    } else {
        push_leaf(
            std::iter::once(data.clone()),
            1,
            depth,
            components,
            depths,
            n_components,
        )
    }
}

/// Validates that a leaf item's component count (`n`) matches every other leaf item's, is
/// nonzero, and is at most 32, then appends its component values and depth.
fn push_leaf<'py>(
    values: impl Iterator<Item = Bound<'py, PyAny>>,
    n: usize,
    depth: u8,
    components: &mut Vec<Bound<'py, PyAny>>,
    depths: &mut Vec<u8>,
    n_components: &mut Option<usize>,
) -> PyResult<()> {
    match *n_components {
        None => {
            if n == 0 {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "Aggregate items (tuples) must not be empty.",
                ));
            }
            if n > 32 {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Aggregate items with more than 32 components are not supported (got {n})."
                )));
            }
            *n_components = Some(n);
        }
        Some(expected) if expected != n => {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "All aggregate items (tuples) in a deck must have the same length.",
            ));
        }
        _ => {}
    }
    components.extend(values);
    depths.push(depth);
    Ok(())
}

fn detect_type(values: &[Bound<'_, PyAny>]) -> PyResult<u64> {
    if values.iter().any(|v| v.is_instance_of::<PyFloat>()) {
        return Ok(ORC_TYPE_F64);
    }
    let mut lo: i128 = i128::MAX;
    let mut hi: i128 = i128::MIN;
    for val in values {
        let v: i128 = if let Ok(i) = val.extract::<i64>() {
            i as i128
        } else {
            val.extract::<u64>()? as i128
        };
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if lo >= 0 {
        if hi <= 0xFF {
            Ok(ORC_TYPE_U8)
        } else if hi <= 0xFFFF {
            Ok(ORC_TYPE_U16)
        } else if hi <= 0xFFFF_FFFF {
            Ok(ORC_TYPE_U32)
        } else {
            Ok(ORC_TYPE_U64)
        }
    } else if lo >= -0x80 && hi <= 0x7F {
        Ok(ORC_TYPE_I8)
    } else if lo >= -0x8000 && hi <= 0x7FFF {
        Ok(ORC_TYPE_I16)
    } else if lo >= -0x8000_0000 && hi <= 0x7FFF_FFFF {
        Ok(ORC_TYPE_I32)
    } else {
        Ok(ORC_TYPE_I64)
    }
}

// =====================================================================
// read_deck helpers
// =====================================================================

/// Reconstruct nested Python lists from flat items and sparse marks.
/// Converts marks to per-value depths, then builds the structure in a
/// single recursive pass.
fn deck_to_nested_py_list(
    py: Python<'_>,
    items: Vec<PyObject>,
    marks: &[OrcMark],
) -> PyResult<PyObject> {
    if marks.is_empty() || items.is_empty() {
        return Ok(PyList::new(py, &items)?.into_any().unbind());
    }
    // Convert sparse marks to per-value depths.
    let mut depths = vec![0u8; items.len()];
    for mark in marks {
        depths[mark.pos as usize] = mark.depth + 1;
    }
    // Build nested lists recursively.
    let mut idx = 0;
    let list = PyList::empty(py);
    build_nested_list(py, &items, &depths, &mut idx, &list, 1)?;
    Ok(list.into_any().unbind())
}

/// Recursive helper: reads items[idx..] and appends nested lists to `dst`.
/// `rdepth` is the nesting depth of `dst` — values with matching depth are
/// appended directly; deeper values trigger sub-list creation.
fn build_nested_list(
    py: Python<'_>,
    items: &[PyObject],
    depths: &[u8],
    idx: &mut usize,
    dst: &Bound<'_, PyList>,
    rdepth: u8,
) -> PyResult<()> {
    if *idx >= items.len() {
        return Ok(());
    }
    let d = depths[*idx];
    if d == rdepth {
        // Terminal: append leaf values until the next marked position.
        loop {
            dst.append(&items[*idx])?;
            *idx += 1;
            if *idx >= items.len() || depths[*idx] != 0 {
                break;
            }
        }
    } else if d > rdepth {
        // Deeper nesting — create sub-lists at this level.
        let gap = d - rdepth;
        let mut next_rdepth = rdepth;
        loop {
            let nested = PyList::empty(py);
            build_nested_list(py, items, depths, idx, &nested, next_rdepth + 1)?;
            dst.append(&nested)?;
            next_rdepth = 0;
            if *idx >= items.len() || depths[*idx] != gap {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_parse_dtype_all_known_types() {
        assert_eq!(parse_dtype("u8").unwrap(), ORC_TYPE_U8);
        assert_eq!(parse_dtype("u16").unwrap(), ORC_TYPE_U16);
        assert_eq!(parse_dtype("u32").unwrap(), ORC_TYPE_U32);
        assert_eq!(parse_dtype("u64").unwrap(), ORC_TYPE_U64);
        assert_eq!(parse_dtype("i8").unwrap(), ORC_TYPE_I8);
        assert_eq!(parse_dtype("i16").unwrap(), ORC_TYPE_I16);
        assert_eq!(parse_dtype("i32").unwrap(), ORC_TYPE_I32);
        assert_eq!(parse_dtype("i64").unwrap(), ORC_TYPE_I64);
        assert_eq!(parse_dtype("f32").unwrap(), ORC_TYPE_F32);
        assert_eq!(parse_dtype("f64").unwrap(), ORC_TYPE_F64);
    }

    #[test]
    fn t_parse_dtype_unknown_returns_err() {
        assert!(parse_dtype("float32").is_err());
        assert!(parse_dtype("int").is_err());
        assert!(parse_dtype("").is_err());
    }
}
