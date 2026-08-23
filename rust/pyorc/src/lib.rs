mod func;
mod graph;
mod handle;
mod host;
mod stubs;

use func::OrcFunc;
use graph::{Graph, GraphNode, GraphNodeKind, IN_GRAPH_MODE, BUILDING_WORKFLOW};
use handle::Handle;
use host::{HANDLE_COUNTER, PLUGIN_SET, REGISTRY};

use orc_sdk::{
    Deck, OrcHandle, OrcMark, ORC_TYPE_F32, ORC_TYPE_F64, ORC_TYPE_I8, ORC_TYPE_I16, ORC_TYPE_I32,
    ORC_TYPE_I64, ORC_TYPE_U8, ORC_TYPE_U16, ORC_TYPE_U32, ORC_TYPE_U64,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyList, PyTuple};
use std::sync::atomic::Ordering;

#[pymodule(name = "orc")]
fn pyorc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Handle>()?;
    m.add_class::<GraphNode>()?;
    m.add_class::<Graph>()?;
    m.add_function(wrap_pyfunction!(load_plugins, m)?)?;
    m.add_function(wrap_pyfunction!(make_deck, m)?)?;
    m.add_function(wrap_pyfunction!(read_deck, m)?)?;
    m.add_function(wrap_pyfunction!(make_graph, m)?)?;
    m.add_function(wrap_pyfunction!(run_graph, m)?)?;
    m.add_function(wrap_pyfunction!(serialize_workflow, m)?)?;
    m.add_function(wrap_pyfunction!(deserialize_workflow, m)?)?;
    // Type constants.
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
    let plugin_set =
        orc_sdk::load_plugins(std::path::Path::new(search_dir), &host::HOST).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to load plugins: {:?}", e))
        })?;

    if PLUGIN_SET.set(plugin_set).is_err() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(
            "load_plugins has already been called",
        ));
    }

    let ps = PLUGIN_SET.get().unwrap();

    // Register plugin functions as module attributes.
    let module = PyModule::import(py, "orc")?;
    for plugin in ps.plugins() {
        for func_info in plugin.functions() {
            let orc_func = OrcFunc {
                info: func_info.clone(),
            };
            let py_func = Py::new(py, orc_func)?;
            module.setattr(pyo3::types::PyString::new(py, &func_info.name), py_func)?;
        }
    }

    // Generate .pyi stubs.
    let _ = stubs::generate_stubs(py, ps); // Best effort — don't fail load on stub error.

    Ok(())
}

#[pyfunction]
#[pyo3(signature = (data, type_id=None))]
fn make_deck(py: Python<'_>, data: &Bound<'_, PyAny>, type_id: Option<u64>) -> PyResult<PyObject> {
    if IN_GRAPH_MODE.load(Ordering::Relaxed) {
        return make_deck_deferred(py, data, type_id);
    }
    let handle = create_orc_handle(py, data, type_id)?;
    Ok(Py::new(py, Handle::new(handle))?.into_any())
}

#[pyfunction]
fn read_deck(py: Python<'_>, handle: &Handle) -> PyResult<PyObject> {
    let h = &handle.inner.0;
    macro_rules! read_typed {
        ($T:ty) => {{
            let items: &[$T] = h.items::<$T>();
            let py_items: Vec<PyObject> = items
                .iter()
                .map(|v| v.into_pyobject(py).unwrap().into_any().unbind())
                .collect();
            reconstruct_nested(py, py_items, h.marks())
        }};
    }
    match h.type_id {
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
            h.type_id
        ))),
    }
}

#[pyfunction]
fn make_graph(py: Python<'_>, func: &Bound<'_, PyAny>) -> PyResult<Graph> {
    graph::make_graph_impl(py, func)
}

#[pyfunction]
#[pyo3(signature = (graph, *args, **kwargs))]
fn run_graph<'py>(
    graph: &Graph,
    py: Python<'py>,
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<PyObject> {
    graph.run_impl(py, args, kwargs)
}

#[pyfunction]
fn serialize_workflow(graph: &Graph, path: &str) -> PyResult<()> {
    graph::serialize_workflow_impl(graph, path)
}

#[pyfunction]
fn deserialize_workflow(py: Python<'_>, path: &str) -> PyResult<Graph> {
    graph::deserialize_workflow_impl(py, path)
}

// =====================================================================
// make_deck internals
// =====================================================================

fn create_orc_handle(
    _py: Python<'_>,
    data: &Bound<'_, PyAny>,
    type_id: Option<u64>,
) -> PyResult<OrcHandle> {
    let mut leaf_values: Vec<Bound<'_, PyAny>> = Vec::new();
    let mut depths: Vec<u8> = Vec::new();

    if data.downcast::<PyList>().is_ok() {
        let depth = intrinsic_depth(data)?;
        collect_leaves(data, depth, &mut leaf_values, &mut depths)?;
    } else {
        leaf_values.push(data.clone());
        depths.push(0);
    }

    let type_id = match type_id {
        Some(id) => id,
        None if leaf_values.is_empty() => ORC_TYPE_F64,
        None => detect_type(&leaf_values)?,
    };

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
                .map_err(|e| {
                    pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
                })?;
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
            )))
        }
    }

    Ok(handle)
}

fn make_deck_deferred(
    py: Python<'_>,
    data: &Bound<'_, PyAny>,
    type_id: Option<u64>,
) -> PyResult<PyObject> {
    // Create the OrcHandle — this is the one place OrcHandle appears in deferred mode.
    let handle = create_orc_handle(py, data, type_id)?;

    // Add as a constant node that owns the handle.
    let mut wf_guard = BUILDING_WORKFLOW
        .lock()
        .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("workflow lock poisoned"))?;
    let wf = &mut wf_guard
        .as_mut()
        .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("no workflow being built"))?
        .0;

    let (_nh, oh) = wf.add_constant(handle).map_err(|e| {
        pyo3::exceptions::PyRuntimeError::new_err(format!("{:?}", e))
    })?;

    Ok(Py::new(py, GraphNode { kind: GraphNodeKind::Output(oh) })?.into_any())
}

// =====================================================================
// Data flattening helpers (mirrors python/orc.py _push / _intrinsic_depth)
// =====================================================================

fn intrinsic_depth(data: &Bound<'_, PyAny>) -> PyResult<u8> {
    if let Ok(list) = data.downcast::<PyList>() {
        if !list.is_empty() {
            let first = list.get_item(0)?;
            if first.downcast::<PyList>().is_ok() {
                return Ok(1 + intrinsic_depth(&first.as_any())?);
            }
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
            // List of lists.
            collect_leaves(&first.as_any(), depth, items, depths)?;
            for i in 1..list.len() {
                let sub = list.get_item(i)?;
                let sub_depth = intrinsic_depth(&sub.as_any())?;
                collect_leaves(&sub.as_any(), sub_depth, items, depths)?;
            }
        } else {
            // Flat list of leaf values.
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
    let has_float = values.iter().any(|v| v.is_instance_of::<PyFloat>());
    if has_float {
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
// read_deck helpers (mirrors python/orc.py read_deck nesting logic)
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

    let marks_data: Vec<(u8, u64)> = marks.iter().map(|m| (m.depth, m.pos)).collect();
    let max_depth = marks_data[0].0 as usize + 1;

    // Split items into leaf groups at mark positions.
    let mut result: Vec<PyObject> = Vec::with_capacity(marks_data.len());
    for i in 0..marks_data.len() {
        let start = marks_data[i].1 as usize;
        let end = if i + 1 < marks_data.len() {
            marks_data[i + 1].1 as usize
        } else {
            items.len()
        };
        let list = PyList::new(py, &items[start..end])?;
        result.push(list.into_any().unbind());
    }

    // Nest bottom-up.
    for d in 1..max_depth {
        let mut boundaries = vec![0usize];
        for i in 1..marks_data.len() {
            if marks_data[i].0 as usize >= d {
                boundaries.push(i);
            }
        }
        boundaries.push(result.len());

        let mut new_result: Vec<PyObject> = Vec::with_capacity(boundaries.len() - 1);
        for b in 0..boundaries.len() - 1 {
            let slice = &result[boundaries[b]..boundaries[b + 1]];
            let list = PyList::new(py, slice)?;
            new_result.push(list.into_any().unbind());
        }
        result = new_result;
    }

    if result.len() == 1 {
        Ok(result.pop().unwrap())
    } else {
        let list = PyList::new(py, &result)?;
        Ok(list.into_any().unbind())
    }
}
