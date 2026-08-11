

import ctypes
import gc
import math
import os
import sys

import numpy as np

import orc

# ---------------------------------------------------------------------------
# Handle leak tracking (test-only)
# ---------------------------------------------------------------------------

_live_handles = set()
_orig_del = orc.OrcHandle.__del__


def _tracking_del(self):
    _live_handles.discard(self.handle)
    _orig_del(self)


orc.OrcHandle.__del__ = _tracking_del


def make_handle(data, type_id=None):
    """Create a handle and register it for leak tracking."""
    h = orc.make_deck(data, type_id=type_id)
    _live_handles.add(h.handle)
    return h


def assert_no_leaks():
    """Assert all tracked handles have been freed."""
    gc.collect()
    assert not _live_handles, f"Leaked handle IDs: {_live_handles}"


# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(script_dir)
search_dir = os.path.join(project_root, "build", "debug")


def call_fn(fn, inputs, n_outputs=1):
    """Call a plugin function by name and return output handles."""
    result = fn(*inputs, n_out=n_outputs)
    if n_outputs == 1:
        _live_handles.add(result.handle)
        return [result]
    for h in result:
        _live_handles.add(h.handle)
    return list(result)


# ============================================================
# add — Correctness
# ============================================================


def t_add_f64_flat():
    """Add two flat f64 lists element-wise."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn(orc.add, [a, b])
    assert orc.read_deck(out) == [11.0, 22.0, 33.0]


def t_add_f64_nested():
    """Add nested list with broadcast flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn(orc.add, [a, b])
    result = orc.read_deck(out)
    assert result == [[11.0, 22.0, 33.0], [12.0, 24.0, 36.0, 38.0]]


def t_add_broadcast_scalar():
    """Add a scalar to each element of a list."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0])
    [out] = call_fn(orc.add, [a, b])
    assert orc.read_deck(out) == [11.0, 12.0, 13.0]


def t_add_single_element():
    """Add two single-element lists."""
    a = make_handle([5.0])
    b = make_handle([3.0])
    [out] = call_fn(orc.add, [a, b])
    assert orc.read_deck(out) == [8.0]


def t_add_depth3():
    """Add a scalar to a depth-3 nested list."""
    a = make_handle([[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]])
    b = make_handle([10.0])
    [out] = call_fn(orc.add, [a, b])
    result = orc.read_deck(out)
    expected = [
        [[11.0, 12.0], [13.0, 14.0]],
        [[15.0, 16.0], [17.0, 18.0]],
    ]
    assert result == expected


# ============================================================
# add — Integer types
# ============================================================


def t_add_i32():
    """Add I8 and U8 lists triggers type mismatch."""
    a = make_handle([-5, -3, 0, 3, 5])
    b = make_handle([10, 20, 30, 40, 50])
    assert a.type_id == orc.ORC_TYPE_I8
    [out] = call_fn(orc.add, [a, b])
    # a is I8 (has negatives), b is U8 (all positive).
    # Type mismatch — output untouched.


def t_add_u8():
    """Add two U8 lists element-wise."""
    a = make_handle([1, 2, 3])
    b = make_handle([10, 20, 30])
    assert a.type_id == orc.ORC_TYPE_U8
    assert b.type_id == orc.ORC_TYPE_U8
    [out] = call_fn(orc.add, [a, b])
    assert orc.read_deck(out) == [11, 22, 33]


def t_add_u32():
    """Add two U32 lists element-wise."""
    a = make_handle([100000, 200000])
    b = make_handle([300000, 400000])
    assert a.type_id == orc.ORC_TYPE_U32
    [out] = call_fn(orc.add, [a, b])
    assert orc.read_deck(out) == [400000, 600000]


# ============================================================
# add — Error cases
# ============================================================


def t_add_mismatched_types():
    """Add U8 and F64 lists produces no output."""
    a = make_handle([1, 2, 3])  # U8
    b = make_handle([1.0, 2.0, 3.0])  # F64
    assert a.type_id != b.type_id
    [out] = call_fn(orc.add, [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# mul — Correctness
# ============================================================


def t_mul_f64():
    """Multiply two flat f64 lists element-wise."""
    a = make_handle([2.0, 3.0, 4.0])
    b = make_handle([5.0, 6.0, 7.0])
    [out] = call_fn(orc.mul, [a, b])
    assert orc.read_deck(out) == [10.0, 18.0, 28.0]


def t_mul_u8():
    """Multiply two U8 lists element-wise."""
    a = make_handle([3, 4])
    b = make_handle([7, 8])
    [out] = call_fn(orc.mul, [a, b])
    assert orc.read_deck(out) == [21, 32]


def t_mul_nested():
    """Multiply nested list by broadcast scalar."""
    a = make_handle([[2.0, 3.0], [4.0]])
    b = make_handle([10.0])
    [out] = call_fn(orc.mul, [a, b])
    assert orc.read_deck(out) == [[20.0, 30.0], [40.0]]


def t_mul_mismatched_types():
    """Multiply U8 and F64 lists produces no output."""
    a = make_handle([2, 3])  # U8
    b = make_handle([3.0, 4.0])  # F64
    [out] = call_fn(orc.mul, [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# sub — Correctness
# ============================================================


def t_sub_f64():
    """Subtract two flat f64 lists element-wise."""
    a = make_handle([10.0, 20.0])
    b = make_handle([3.0, 7.0])
    [out] = call_fn(orc.sub, [a, b])
    assert orc.read_deck(out) == [7.0, 13.0]


def t_sub_nested():
    """Subtract broadcast scalar from nested list."""
    a = make_handle([[10.0, 20.0], [30.0]])
    b = make_handle([1.0])
    [out] = call_fn(orc.sub, [a, b])
    assert orc.read_deck(out) == [[9.0, 19.0], [29.0]]


def t_sub_unsupported_type():
    """Sub on U8 is unsupported, produces no output."""
    a = make_handle([5, 3])  # U8
    b = make_handle([3, 1])
    [out] = call_fn(orc.sub, [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# div — Correctness
# ============================================================


def t_div_f64():
    """Divide two flat f64 lists element-wise."""
    a = make_handle([10.0, 9.0])
    b = make_handle([2.0, 3.0])
    [out] = call_fn(orc.div, [a, b])
    assert orc.read_deck(out) == [5.0, 3.0]


def t_div_by_zero():
    """Divide by zero produces positive infinity."""
    a = make_handle([1.0])
    b = make_handle([0.0])
    [out] = call_fn(orc.div, [a, b])
    result = orc.read_deck(out)
    assert math.isinf(result[0]) and result[0] > 0


def t_div_unsupported_type():
    """Div on U8 is unsupported, produces no output."""
    a = make_handle([6, 2])  # U8
    b = make_handle([2, 1])
    [out] = call_fn(orc.div, [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# repeat_list — Correctness
# ============================================================


def t_repeat_list_f64():
    """Repeat a flat f64 list 3 times."""
    a = make_handle([1.0, 2.0, 3.0])
    count = make_handle(3, type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    result = orc.read_deck(out)
    assert result == [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]


def t_repeat_list_u8():
    """Repeat a U8 list 2 times."""
    a = make_handle([10, 20])
    count = make_handle([2], type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    # We expect a nested list because the second input is a list.
    assert orc.read_deck(out) == [[10, 20, 10, 20]]


def t_repeat_list_single_element():
    """Repeat a single-element list 4 times."""
    a = make_handle([42.0])
    count = make_handle([4], type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    assert orc.read_deck(out) == [[42.0, 42.0, 42.0, 42.0]]


def t_repeat_list_one_repeat():
    """Repeating once returns the same items."""
    a = make_handle([5.0, 6.0])
    count = make_handle(1, type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    assert orc.read_deck(out) == [5.0, 6.0]


def t_repeat_list_zero_repeats():
    """Repeating zero times produces an empty output."""
    a = make_handle([1.0, 2.0])
    count = make_handle(0, type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    assert orc.read_deck(out) == []


def t_repeat_list_output_type_matches_input():
    """Output type matches the list input type."""
    a = make_handle([100000, 200000])  # U32
    assert a.type_id == orc.ORC_TYPE_U32
    count = make_handle(2, type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    assert out.type_id == orc.ORC_TYPE_U32
    assert orc.read_deck(out) == [100000, 200000, 100000, 200000]


def t_repeat_list_output_free_fn_set():
    """Plugin output has free_fn set."""
    a = make_handle([1.0])
    count = make_handle(1, type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    assert out.free_fn


def t_repeat_list_nested_input():
    """Repeat each sublist of a nested input."""
    a = make_handle([[1.0, 2.0], [3.0]])
    count = make_handle([2], type_id=orc.ORC_TYPE_U64)
    [out] = call_fn(orc.repeat_list, [a, count])
    result = orc.read_deck(out)
    # Each sublist repeated: [1,2,1,2] and [3,3]
    assert result == [[1.0, 2.0, 1.0, 2.0], [3.0, 3.0]]


# ============================================================
# list_length — Correctness
# ============================================================


def t_list_length_basic():
    """List length of two sublists returns their sizes."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn(orc.list_length, [a])
    result = orc.read_deck(out)
    assert result == [3, 2]
    assert out.type_id == orc.ORC_TYPE_U64


def t_list_length_single_list():
    """List length of one single-element sublist."""
    a = make_handle([[42.0]])
    [out] = call_fn(orc.list_length, [a])
    assert orc.read_deck(out) == [1]


def t_list_length_depth3():
    """List length at depth-3 returns nested lengths."""
    a = make_handle([[[1.0, 2.0], [3.0]], [[4.0, 5.0, 6.0]]])
    [out] = call_fn(orc.list_length, [a])
    result = orc.read_deck(out)
    assert result == [[2, 1], [3]]


# ============================================================
# flatten_deck — Correctness
# ============================================================


def t_flatten_basic():
    """Flatten a nested list into a flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn(orc.flatten_deck, [a])
    result = orc.read_deck(out)
    assert result == [1.0, 2.0, 3.0, 4.0, 5.0]


def t_flatten_already_flat():
    """Flatten an already-flat list is a no-op."""
    a = make_handle([1.0, 2.0, 3.0])
    [out] = call_fn(orc.flatten_deck, [a])
    assert orc.read_deck(out) == [1.0, 2.0, 3.0]


def t_flatten_multiple_io():
    """Flatten with multiple inputs and outputs."""
    a = make_handle([[1.0, 2.0], [3.0]])
    b = make_handle([[4.0, 5.0, 6.0]])
    outs = call_fn(orc.flatten_deck, [a, b], n_outputs=2)
    assert orc.read_deck(outs[0]) == [1.0, 2.0, 3.0]
    assert orc.read_deck(outs[1]) == [4.0, 5.0, 6.0]


def t_flatten_integer_type():
    """Flatten preserves the integer element type."""
    a = make_handle([[10, 20], [30]])
    assert a.type_id == orc.ORC_TYPE_U8
    [out] = call_fn(orc.flatten_deck, [a])
    assert out.type_id == orc.ORC_TYPE_U8
    assert orc.read_deck(out) == [10, 20, 30]


# ============================================================
# flatten_deck — Error cases
# ============================================================


def t_flatten_mismatched_counts():
    """Flatten with n_inputs != n_outputs produces no output."""
    a = make_handle([[1.0]])
    out = orc.flatten_deck(a, a, n_out = 1)
    assert not out.items
    assert not out.free_fn


# ============================================================
# Ownership / lifetime invariants
# ============================================================


def t_output_free_fn_set():
    """Plugin output handles have a free_fn set."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    [out] = call_fn(orc.add, [a, b])
    assert out.free_fn


def t_output_handle_id_preserved():
    """Plugin preserves the handle ID set by the caller."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    expected_out_id = orc.next_handle_id() + 1
    out = orc.add(a, b)
    assert out.handle == expected_out_id


def t_output_type_matches_input_for_flatten():
    """Flatten output type matches the input type."""
    a = make_handle([[1.0, 2.0]])
    assert a.type_id == orc.ORC_TYPE_F64
    [out] = call_fn(orc.flatten_deck, [a])
    assert out.type_id == orc.ORC_TYPE_F64


def t_list_length_output_is_u64():
    """List length always outputs U64."""
    a = make_handle([[1.0, 2.0]])
    [out] = call_fn(orc.list_length, [a])
    assert out.type_id == orc.ORC_TYPE_U64


# ============================================================
# make_handle / read_handle roundtrip
# ============================================================


def t_roundtrip_flat():
    """Roundtrip a flat list through make/read."""
    data = [1.0, 2.0, 3.0]
    h = make_handle(data)
    assert orc.read_deck(h) == data


def t_roundtrip_nested():
    """Roundtrip a nested list through make/read."""
    data = [[1.0, 2.0], [3.0, 4.0]]
    h = make_handle(data)
    assert orc.read_deck(h) == data


def t_roundtrip_depth3():
    """Roundtrip a depth-3 nested list through make/read."""
    data = [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]]
    h = make_handle(data)
    assert orc.read_deck(h) == data


def t_roundtrip_ragged():
    """Roundtrip a ragged nested list through make/read."""
    data = [[1.0, 2.0, 3.0], [4.0, 5.0]]
    h = make_handle(data)
    assert orc.read_deck(h) == data


def t_roundtrip_single_element():
    """Roundtrip a single-element list through make/read."""
    data = [42.0]
    h = make_handle(data)
    assert orc.read_deck(h) == data


def t_type_detection_u8():
    """Values in [0, 255] are detected as U8."""
    h = make_handle([0, 127, 255])
    assert h.type_id == orc.ORC_TYPE_U8


def t_type_detection_u16():
    """Values exceeding U8 range are detected as U16."""
    h = make_handle([0, 256])
    assert h.type_id == orc.ORC_TYPE_U16


def t_type_detection_u32():
    """Values exceeding U16 range are detected as U32."""
    h = make_handle([0, 0x10000])
    assert h.type_id == orc.ORC_TYPE_U32


def t_type_detection_u64():
    """Values exceeding U32 range are detected as U64."""
    h = make_handle([0, 0x100000000])
    assert h.type_id == orc.ORC_TYPE_U64


def t_type_detection_i8():
    """Signed values in [-128, 127] are detected as I8."""
    h = make_handle([-128, 127])
    assert h.type_id == orc.ORC_TYPE_I8


def t_type_detection_i16():
    """Signed values exceeding I8 range are detected as I16."""
    h = make_handle([-129, 0])
    assert h.type_id == orc.ORC_TYPE_I16


def t_type_detection_i32():
    """Signed values exceeding I16 range are detected as I32."""
    h = make_handle([-0x8000_0000, 0])
    assert h.type_id == orc.ORC_TYPE_I32


def t_type_detection_i64():
    """Signed values exceeding I32 range are detected as I64."""
    h = make_handle([-0x8000_0001, 0])
    assert h.type_id == orc.ORC_TYPE_I64


def t_type_detection_f64():
    """Any float value triggers F64 detection."""
    h = make_handle([1.0, 2, 3])
    assert h.type_id == orc.ORC_TYPE_F64


# ============================================================
# as_numpy — Zero-copy numpy integration
# ============================================================


def t_numpy_f64():
    """Convert f64 handle to numpy, verify pointer and arithmetic."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn(orc.add, [a, b])
    arr = orc.as_numpy(out)
    assert arr.dtype == np.float64
    assert arr.ctypes.data == out.items
    assert list(arr * 2.0) == [22.0, 44.0, 66.0]


def t_numpy_f32():
    """Convert f32 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([1.5, 2.5, 3.5], type_id=orc.ORC_TYPE_F32)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.float32
    assert arr.ctypes.data == h.items
    result = arr + np.float32(0.5)
    assert list(result) == [2.0, 3.0, 4.0]


def t_numpy_u8():
    """Convert u8 handle to numpy, verify pointer and arithmetic."""
    a = make_handle([10, 20, 30])
    b = make_handle([1, 2, 3])
    [out] = call_fn(orc.add, [a, b])
    arr = orc.as_numpy(out)
    assert arr.dtype == np.uint8
    assert arr.ctypes.data == out.items
    assert list(arr + np.uint8(100)) == [111, 122, 133]


def t_numpy_u16():
    """Convert u16 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([300, 400, 500], type_id=orc.ORC_TYPE_U16)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.uint16
    assert arr.ctypes.data == h.items
    assert list(arr - np.uint16(100)) == [200, 300, 400]


def t_numpy_u32():
    """Convert u32 handle to numpy, verify pointer and arithmetic."""
    a = make_handle([100000, 200000])
    b = make_handle([300000, 400000])
    [out] = call_fn(orc.add, [a, b])
    arr = orc.as_numpy(out)
    assert arr.dtype == np.uint32
    assert arr.ctypes.data == out.items
    assert list(arr // np.uint32(100000)) == [4, 6]


def t_numpy_u64():
    """Convert u64 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([2**40, 2**41], type_id=orc.ORC_TYPE_U64)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.uint64
    assert arr.ctypes.data == h.items
    assert list(arr * np.uint64(2)) == [2**41, 2**42]


def t_numpy_i8():
    """Convert i8 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([-10, 0, 10], type_id=orc.ORC_TYPE_I8)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.int8
    assert arr.ctypes.data == h.items
    assert list(arr + np.int8(5)) == [-5, 5, 15]


def t_numpy_i16():
    """Convert i16 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([-1000, 0, 1000], type_id=orc.ORC_TYPE_I16)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.int16
    assert arr.ctypes.data == h.items
    assert list(arr * np.int16(-1)) == [1000, 0, -1000]


def t_numpy_i32():
    """Convert i32 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([-100000, 0, 100000], type_id=orc.ORC_TYPE_I32)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.int32
    assert arr.ctypes.data == h.items
    assert list(arr + np.int32(1)) == [-99999, 1, 100001]


def t_numpy_i64():
    """Convert i64 handle to numpy, verify pointer and arithmetic."""
    h = make_handle([-2**40, 0, 2**40], type_id=orc.ORC_TYPE_I64)
    arr = orc.as_numpy(h)
    assert arr.dtype == np.int64
    assert arr.ctypes.data == h.items
    assert list(arr + np.int64(1)) == [-(2**40) + 1, 1, 2**40 + 1]


def t_numpy_3x3_matrix():
    """Convert a 3x3 nested handle to a numpy matrix, verify arithmetic."""
    data = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    h = make_handle(data)
    arr = orc.as_numpy(h)
    assert arr.ctypes.data == h.items
    # Flat view of 9 items
    assert len(arr) == 9
    mat = arr.reshape(3, 3)
    # Matrix multiply with identity should return the same matrix
    identity = np.eye(3)
    result = mat @ identity
    assert np.array_equal(result, mat)
    # Sum of each row
    row_sums = mat.sum(axis=1)
    assert list(row_sums) == [6.0, 15.0, 24.0]


def t_numpy_3x3_from_plugin():
    """Plugin output of nested 3x3, converted to numpy matrix."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
    b = make_handle([1.0])
    [out] = call_fn(orc.add, [a, b])
    arr = orc.as_numpy(out)
    assert arr.ctypes.data == out.items
    mat = arr.reshape(3, 3)
    # Each element should be original + 1
    expected = np.array([[2, 3, 4], [5, 6, 7], [8, 9, 10]], dtype=np.float64)
    assert np.array_equal(mat, expected)
    # Column means
    col_means = mat.mean(axis=0)
    assert list(col_means) == [5.0, 6.0, 7.0]


def t_numpy_survives_handle_gc():
    """Numpy array must remain valid after the handle is GC'd."""

    def get_arr():
        h = make_handle([1.0, 2.0, 3.0])
        return orc.as_numpy(h)

    arr = get_arr()
    gc.collect()  # Force GC of the handle
    # If the handle was freed, this reads freed memory.
    assert list(arr) == [1.0, 2.0, 3.0]
    assert list(arr + 1.0) == [2.0, 3.0, 4.0]


def t_numpy_and_handle_both_freed():
    """Handle is freed when both numpy array and handle go out of scope."""

    def create_and_drop():
        h = make_handle([1.0, 2.0, 3.0])
        arr = orc.as_numpy(h)
        assert list(arr) == [1.0, 2.0, 3.0]
        # Both h and arr go out of scope here.

    create_and_drop()
    gc.collect()
    # assert_no_leaks runs after this test returns, verifying the handle
    # was freed even though the numpy array extended its lifetime.


# ============================================================
# complex numbers — create_complex / add_complex / mul_complex
# ============================================================


def make_complex(real_data, imag_data):
    """Create a Complex deck from real and imaginary f64 lists."""
    real = make_handle(real_data)
    imag = make_handle(imag_data)
    [out] = call_fn(orc.create_complex, [real, imag])
    return out


def get_parts(complex_handle):
    """Extract real and imaginary parts from a Complex deck as f64 lists."""
    [real_out, imag_out] = call_fn(orc.complex_get_parts, [complex_handle], n_outputs=2)
    return orc.read_deck(real_out), orc.read_deck(imag_out)


# ==================== add_complex ====================


def t_complex_add_flat():
    """[1+2i, 3+4i] + [10+20i, 30+40i] = [11+22i, 33+44i]"""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    rhs = make_complex([10.0, 30.0], [20.0, 40.0])
    [out] = call_fn(orc.add_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [11.0, 33.0]
    assert imag == [22.0, 44.0]


def t_complex_add_nested():
    """[[1+i, 2+i], [3+i]] + [[4+i, 5+i], [6+i]] = [[5+2i, 7+2i], [9+2i]]"""
    lhs = make_complex([[1.0, 2.0], [3.0]], [[1.0, 1.0], [1.0]])
    rhs = make_complex([[4.0, 5.0], [6.0]], [[1.0, 1.0], [1.0]])
    [out] = call_fn(orc.add_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [[5.0, 7.0], [9.0]]
    assert imag == [[2.0, 2.0], [2.0]]
    assert out.n_marks > 0


def t_complex_add_negative_components():
    """[1-2i, -3+4i] + [0+3i, 3-4i] = [1+1i, 0+0i]"""
    lhs = make_complex([1.0, -3.0], [-2.0, 4.0])
    rhs = make_complex([0.0, 3.0], [3.0, -4.0])
    [out] = call_fn(orc.add_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [1.0, 0.0]
    assert imag == [1.0, 0.0]


def t_complex_add_wrong_n_inputs():
    """add_complex raises ValueError when called with the wrong number of inputs."""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    try:
        [out] = call_fn(orc.add_complex, [lhs])
    except ValueError:
        # We supplied the wrong number of inputs, so this is expected.
        return
    assert False, "This should be unreachable"


# ==================== mul_complex ====================


def t_complex_mul_flat():
    """[1+2i, 2+3i] * [3+4i, 1+0i] = [-5+10i, 2+3i]"""
    lhs = make_complex([1.0, 2.0], [2.0, 3.0])
    rhs = make_complex([3.0, 1.0], [4.0, 0.0])
    [out] = call_fn(orc.mul_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [-5.0, 2.0]
    assert imag == [10.0, 3.0]


def t_complex_mul_nested():
    """[[1+0i, 0+1i], [2+0i]] * [[5+0i, 0+1i], [3+0i]] = [[5+0i, -1+0i], [6+0i]]"""
    lhs = make_complex([[1.0, 0.0], [2.0]], [[0.0, 1.0], [0.0]])
    rhs = make_complex([[5.0, 0.0], [3.0]], [[0.0, 1.0], [0.0]])
    [out] = call_fn(orc.mul_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [[5.0, -1.0], [6.0]]
    assert imag == [[0.0, 0.0], [0.0]]
    assert out.n_marks > 0


def t_complex_mul_i_squared():
    """[0+1i, 0+1i, 0+1i] * [0+1i, 0+1i, 0+1i] = [-1+0i, -1+0i, -1+0i]"""
    lhs = make_complex([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    rhs = make_complex([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    [out] = call_fn(orc.mul_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [-1.0, -1.0, -1.0]
    assert imag == [0.0, 0.0, 0.0]


def t_complex_mul_by_zero():
    """[3+4i, 1+1i] * [0+0i, 0+0i] = [0+0i, 0+0i]"""
    lhs = make_complex([3.0, 1.0], [4.0, 1.0])
    rhs = make_complex([0.0, 0.0], [0.0, 0.0])
    [out] = call_fn(orc.mul_complex, [lhs, rhs])
    real, imag = get_parts(out)
    assert real == [0.0, 0.0]
    assert imag == [0.0, 0.0]


def t_complex_mul_wrong_n_inputs():
    """mul_complex raises ValueError when called with the wrong number of inputs."""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    try:
        [out] = call_fn(orc.mul_complex, [lhs])
    except ValueError:
        # We supplied the wrong number of inputs, so this is expected.
        return
    assert False, "This should be unreachable"


def t_complex_flatten():
    """Flatten a nested complex deck into a flat list."""
    nested = make_complex([[1.0, 3.0], [5.0]], [[2.0, 4.0], [6.0]])
    [flat] = call_fn(orc.flatten_deck, [nested])
    real, imag = get_parts(flat)
    assert real == [1.0, 3.0, 5.0]
    assert imag == [2.0, 4.0, 6.0]
    assert flat.n_marks == 1
    assert flat.marks[0].depth == 0
    assert flat.marks[0].pos == 0


# ============================================================
# Runner
# ============================================================

if __name__ == "__main__":
    # by convention all tests start with _t
    orc.load_plugins(search_dir)
    tests = [(name, fn) for name, fn in globals().items()
             if name.startswith("t_") and callable(fn)]
    tests.sort(key=lambda x: x[0])
    passed = 0
    failed = 0
    for name, fn in tests:
        try:
            fn()
            assert_no_leaks()
            passed += 1
            print(f"  PASS  {name}")
        except Exception:
            import traceback
            _live_handles.clear()
            failed += 1
            print(f"  FAIL  {name}")
            traceback.print_exc()
    print(f"\n{passed} passed, {failed} failed, {passed + failed} total")
    sys.exit(1 if failed else 0)
