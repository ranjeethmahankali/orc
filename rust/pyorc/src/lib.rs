mod func;
mod graph;
mod handle;
mod host;
mod stubs;

use func::OrcFunc;
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
    m.add_function(wrap_pyfunction!(load_plugins, m)?)?;
    m.add_function(wrap_pyfunction!(make_deck, m)?)?;
    m.add_function(wrap_pyfunction!(read_deck, m)?)?;
    m.add_function(wrap_pyfunction!(make_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(run_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(save_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(load_workflow, m)?)?;
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
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to load plugins: {:?}.", e))
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
    if IN_WORKFLOW_MODE.load(Ordering::Relaxed) {
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

#[pyfunction]
fn read_deck(py: Python<'_>, handle: &Handle) -> PyResult<PyObject> {
    let handle = &handle.inner;
    macro_rules! read_typed {
        ($T:ty) => {{
            let items: &[$T] = h.items::<$T>();
            let py_items: Vec<PyObject> = items
                .iter()
                .map(|v| Ok(v.into_pyobject(py)?.into_any().unbind()))
                .collect::<PyResult<_>>()?;
            reconstruct_nested(py, py_items, h.marks())
        }};
    }
    match handle.type_id {
        ORC_TYPE_U8 => read_typed!(u8),
        ORC_TYPE_U16 => read_typed!(u16),
        ORC_TYPE_U32 => read_typed!(u32),
        ORC_TYPE_U64 => read_typed!(u64),
        ORC_TYPE_I8 => read_typed!(i8),
        ORC_TYPE_I16 => read_typed!(i16),
        ORC_TYPE_I32 => read_typed!(i32),
        ORC_TYPE_I64 => read_typed!(i64),
        ORC_TYPE_F32 => read_typed!(f32),
        ORC_TYPE_F64 => read_typed!(f64),
        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Unknown type_id: {:#x}",
            handle.type_id
        ))),
    }
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
    // Flatten data into leaf values and their mark depths.
    let mut leaf_values: Vec<Bound<'_, PyAny>> = Vec::new();
    let mut depths: Vec<u8> = Vec::new();
    if data.downcast::<PyList>().is_ok() {
        let depth = intrinsic_depth(data)?;
        collect_leaves(data, depth, &mut leaf_values, &mut depths)?;
    } else {
        leaf_values.push(data.clone());
        depths.push(0);
    }
    // Detect or use the provided type.
    let type_id = match type_id {
        Some(id) => id,
        None if leaf_values.is_empty() => ORC_TYPE_F64,
        None => detect_type(&leaf_values)?,
    };
    // Build a typed Deck and allocate in the host registry.
    let mut handle = OrcHandle {
        handle: HANDLE_COUNTER.fetch_add(1, Ordering::Relaxed),
        ..Default::default()
    };
    macro_rules! build_deck {
        ($T:ty) => {{
            let mut deck = Deck::<$T>::default();
            for (val, &depth) in leaf_values.iter().zip(depths.iter()) {
                let v: $T = val.extract()?;
                deck.push(v, depth);
            }
            REGISTRY
                .alloc_with_value(Some(deck), &mut handle)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?;
        }};
    }
    match type_id {
        ORC_TYPE_U8 => build_deck!(u8),
        ORC_TYPE_U16 => build_deck!(u16),
        ORC_TYPE_U32 => build_deck!(u32),
        ORC_TYPE_U64 => build_deck!(u64),
        ORC_TYPE_I8 => build_deck!(i8),
        ORC_TYPE_I16 => build_deck!(i16),
        ORC_TYPE_I32 => build_deck!(i32),
        ORC_TYPE_I64 => build_deck!(i64),
        ORC_TYPE_F32 => build_deck!(f32),
        ORC_TYPE_F64 => build_deck!(f64),
        _ => {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Unsupported type_id: {:#x}",
                type_id
            )));
        }
    }
    Ok(handle)
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
    let wf: &mut Workflow = wf_guard
        .as_mut()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("No workflow being built"))?;
    let oh = wf
        .add_constant(handle)
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e)))?
        .1;
    Ok(Py::new(
        py,
        WorkflowNode {
            kind: WorkflowNodeKind::Output(oh),
        },
    )?
    .into_any())
}

// =====================================================================
// Data flattening helpers
// =====================================================================

fn intrinsic_depth(data: &Bound<'_, PyAny>) -> PyResult<u8> {
    if let Ok(list) = data.downcast::<PyList>()
        && !list.is_empty()
    {
        let first = list.get_item(0)?;
        if first.downcast::<PyList>().is_ok() {
            return Ok(1 + intrinsic_depth(first.as_any())?);
        }
    }
    Ok(1)
}

fn collect_leaves<'py>(
    data: &Bound<'py, PyAny>,
    depth: u8,
    items: &mut Vec<Bound<'py, PyAny>>,
    depths: &mut Vec<u8>,
) -> PyResult<()> {
    if let Ok(list) = data.downcast::<PyList>() {
        if list.is_empty() {
            return Ok(());
        }
        let first = list.get_item(0)?;
        if first.downcast::<PyList>().is_ok() {
            collect_leaves(first.as_any(), depth, items, depths)?;
            for i in 1..list.len() {
                let sub = list.get_item(i)?;
                let sub_depth = intrinsic_depth(sub.as_any())?;
                collect_leaves(sub.as_any(), sub_depth, items, depths)?;
            }
        } else {
            for i in 0..list.len() {
                let val = list.get_item(i)?;
                let d = if i == 0 { depth } else { 0 };
                depths.push(d);
                items.push(val.into_any());
            }
        }
    } else {
        depths.push(depth);
        items.push(data.clone());
    }
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

fn reconstruct_nested(
    py: Python<'_>,
    items: Vec<PyObject>,
    marks: &[OrcMark],
) -> PyResult<PyObject> {
    if marks.is_empty() {
        let list = PyList::new(py, &items)?;
        return Ok(list.into_any().unbind());
    }
    let max_depth = marks[0].depth as usize + 1;
    // Split items into leaf groups between consecutive mark positions.
    let positions: Vec<usize> = marks.iter().map(|m| m.pos as usize).collect();
    let mut result: Vec<PyObject> = positions
        .windows(2)
        .map(|w| PyList::new(py, &items[w[0]..w[1]]).map(|l| l.into_any().unbind()))
        .collect::<PyResult<_>>()?;
    let last = PyList::new(py, &items[*positions.last().unwrap()..])?
        .into_any()
        .unbind();
    result.push(last);
    // Nest bottom-up by depth.
    for d in 1..max_depth {
        let mut boundaries: Vec<usize> = vec![0];
        boundaries.extend(
            marks
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, m)| m.depth as usize >= d)
                .map(|(i, _)| i),
        );
        boundaries.push(result.len());
        result = boundaries
            .windows(2)
            .map(|w| PyList::new(py, &result[w[0]..w[1]]).map(|l| l.into_any().unbind()))
            .collect::<PyResult<_>>()?;
    }
    if result.len() == 1 {
        Ok(result.pop().unwrap())
    } else {
        let list = PyList::new(py, &result)?;
        Ok(list.into_any().unbind())
    }
}
