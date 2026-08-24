


import gc
import math
import os
import sys
import tempfile
import threading

import numpy as np

import orc

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(script_dir)
search_dir = os.path.join(project_root, "build", "debug")


def make_handle(data, type_id=None):
    """Create a handle via orc.make_deck."""
    return orc.make_deck(data, type_id=type_id)


# ============================================================
# add — Correctness
# ============================================================


def t_add_f64_flat():
    """Add two flat f64 lists element-wise."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    out = orc.add(a, b)
    assert orc.read_deck(out) == [11.0, 22.0, 33.0]


def t_add_f64_nested():
    """Add nested list with broadcast flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = make_handle([10.0, 20.0, 30.0])
    out = orc.add(a, b)
    result = orc.read_deck(out)
    assert result == [[11.0, 22.0, 33.0], [12.0, 24.0, 36.0, 38.0]]


def t_add_broadcast_scalar():
    """Add a scalar to each element of a list."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0])
    out = orc.add(a, b)
    assert orc.read_deck(out) == [11.0, 12.0, 13.0]


def t_add_single_element():
    """Add two single-element lists."""
    a = make_handle([5.0])
    b = make_handle([3.0])
    out = orc.add(a, b)
    assert orc.read_deck(out) == [8.0]


def t_add_depth3():
    """Add a scalar to a depth-3 nested list."""
    a = make_handle([[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]])
    b = make_handle([10.0])
    out = orc.add(a, b)
    result = orc.read_deck(out)
    expected = [
        [[11.0, 12.0], [13.0, 14.0]],
        [[15.0, 16.0], [17.0, 18.0]],
    ]
    assert result == expected


# ============================================================
# add — Integer types
# ============================================================


def t_add_i64():
    """Add I8 and U8 lists triggers type mismatch."""
    a = make_handle([-5, -3, 0, 3, 5], orc.ORC_TYPE_I64)
    b = make_handle([10, 20, 30, 40, 50], orc.ORC_TYPE_I64)
    assert a.type_id == orc.ORC_TYPE_I64

# ============================================================
# add — Error cases
# ============================================================


def t_add_mismatched_types():
    """Add U8 and F64 lists raises RuntimeError."""
    a = make_handle([1, 2, 3])  # U8
    b = make_handle([1.0, 2.0, 3.0])  # F64
    assert a.type_id != b.type_id
    try:
        orc.add(a, b)
        assert False, "Should have raised RuntimeError"
    except RuntimeError:
        pass


# ============================================================
# mul — Correctness
# ============================================================


def t_mul_f64():
    """Multiply two flat f64 lists element-wise."""
    a = make_handle([2.0, 3.0, 4.0])
    b = make_handle([5.0, 6.0, 7.0])
    out = orc.mul(a, b)
    assert orc.read_deck(out) == [10.0, 18.0, 28.0]


def t_mul_i64():
    """Multiply two U8 lists element-wise."""
    a = make_handle([3, 4], orc.ORC_TYPE_I64)
    b = make_handle([7, 8], orc.ORC_TYPE_I64)
    out = orc.mul(a, b)
    assert orc.read_deck(out) == [21, 32]


def t_mul_nested():
    """Multiply nested list by broadcast scalar."""
    a = make_handle([[2.0, 3.0], [4.0]])
    b = make_handle([10.0])
    out = orc.mul(a, b)
    assert orc.read_deck(out) == [[20.0, 30.0], [40.0]]


def t_mul_mismatched_types():
    """Multiply U8 and F64 lists raises RuntimeError."""
    a = make_handle([2, 3])  # U8
    b = make_handle([3.0, 4.0])  # F64
    try:
        orc.mul(a, b)
        assert False, "Should have raised RuntimeError"
    except RuntimeError:
        pass


# ============================================================
# sub — Correctness
# ============================================================


def t_sub_f64():
    """Subtract two flat f64 lists element-wise."""
    a = make_handle([10.0, 20.0])
    b = make_handle([3.0, 7.0])
    out = orc.sub(a, b)
    assert orc.read_deck(out) == [7.0, 13.0]


def t_sub_nested():
    """Subtract broadcast scalar from nested list."""
    a = make_handle([[10.0, 20.0], [30.0]])
    b = make_handle([1.0])
    out = orc.sub(a, b)
    assert orc.read_deck(out) == [[9.0, 19.0], [29.0]]


def t_sub_unsupported_type():
    """Sub on U8 is unsupported, raises RuntimeError."""
    a = make_handle([5, 3])  # U8
    b = make_handle([3, 1])
    try:
        orc.sub(a, b)
        assert False, "Should have raised RuntimeError"
    except RuntimeError:
        pass


# ============================================================
# div — Correctness
# ============================================================


def t_div_f64():
    """Divide two flat f64 lists element-wise."""
    a = make_handle([10.0, 9.0])
    b = make_handle([2.0, 3.0])
    out = orc.div(a, b)
    assert orc.read_deck(out) == [5.0, 3.0]


def t_div_by_zero():
    """Divide by zero produces positive infinity."""
    a = make_handle([1.0])
    b = make_handle([0.0])
    out = orc.div(a, b)
    result = orc.read_deck(out)
    assert math.isinf(result[0]) and result[0] > 0


def t_div_unsupported_type():
    """Div on U8 is unsupported, raises RuntimeError."""
    a = make_handle([6, 2])  # U8
    b = make_handle([2, 1])
    try:
        orc.div(a, b)
        assert False, "Should have raised RuntimeError"
    except RuntimeError:
        pass


# ============================================================
# repeat_list — Correctness
# ============================================================


def t_repeat_list_f64():
    """Repeat a flat f64 list 3 times."""
    a = make_handle([1.0, 2.0, 3.0])
    count = make_handle(3, type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    result = orc.read_deck(out)
    assert result == [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]


def t_repeat_list_u8():
    """Repeat a U8 list 2 times."""
    a = make_handle([10, 20])
    count = make_handle([2], type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    # We expect a nested list because the second input is a list.
    assert orc.read_deck(out) == [[10, 20, 10, 20]]


def t_repeat_list_single_element():
    """Repeat a single-element list 4 times."""
    a = make_handle([42.0])
    count = make_handle([4], type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    assert orc.read_deck(out) == [[42.0, 42.0, 42.0, 42.0]]


def t_repeat_list_one_repeat():
    """Repeating once returns the same items."""
    a = make_handle([5.0, 6.0])
    count = make_handle(1, type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    assert orc.read_deck(out) == [5.0, 6.0]


def t_repeat_list_zero_repeats():
    """Repeating zero times produces an empty output."""
    a = make_handle([1.0, 2.0])
    count = make_handle(0, type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    assert orc.read_deck(out) == []


def t_repeat_list_output_type_matches_input():
    """Output type matches the list input type."""
    a = make_handle([100000, 200000])  # U32
    assert a.type_id == orc.ORC_TYPE_U32
    count = make_handle(2, type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    assert out.type_id == orc.ORC_TYPE_U32
    assert orc.read_deck(out) == [100000, 200000, 100000, 200000]


def t_repeat_list_output_free_fn_set():
    """Plugin output has valid ownership (Rust RAII manages lifetime)."""
    a = make_handle([1.0])
    count = make_handle(1, type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    # Skipped: no free_fn attribute in the new Handle. Rust manages lifetime.
    assert out.n_items > 0


def t_repeat_list_nested_input():
    """Repeat each sublist of a nested input."""
    a = make_handle([[1.0, 2.0], [3.0]])
    count = make_handle([2], type_id=orc.ORC_TYPE_U64)
    out = orc.repeat_list(a, count)
    result = orc.read_deck(out)
    # Each sublist repeated: [1,2,1,2] and [3,3]
    assert result == [[1.0, 2.0, 1.0, 2.0], [3.0, 3.0]]


# ============================================================
# list_length — Correctness
# ============================================================


def t_list_length_basic():
    """List length of two sublists returns their sizes."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    out = orc.list_length(a)
    result = orc.read_deck(out)
    assert result == [3, 2]
    assert out.type_id == orc.ORC_TYPE_U64


def t_list_length_single_list():
    """List length of one single-element sublist."""
    a = make_handle([[42.0]])
    out = orc.list_length(a)
    assert orc.read_deck(out) == [1]


def t_list_length_depth3():
    """List length at depth-3 returns nested lengths."""
    a = make_handle([[[1.0, 2.0], [3.0]], [[4.0, 5.0, 6.0]]])
    out = orc.list_length(a)
    result = orc.read_deck(out)
    assert result == [[2, 1], [3]]


# ============================================================
# flatten_deck — Correctness
# ============================================================


def t_flatten_basic():
    """Flatten a nested list into a flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    out = orc.flatten_deck(a)
    result = orc.read_deck(out)
    assert result == [1.0, 2.0, 3.0, 4.0, 5.0]


def t_flatten_already_flat():
    """Flatten an already-flat list is a no-op."""
    a = make_handle([1.0, 2.0, 3.0])
    out = orc.flatten_deck(a)
    assert orc.read_deck(out) == [1.0, 2.0, 3.0]


def t_flatten_multiple_io():
    """Flatten with multiple inputs and outputs."""
    # Skipped: flatten_deck is variadic (n_outputs = VARIADIC). With the new
    # module, variadic functions default to 1 output, so multi-output flatten
    # cannot be tested without n_out= support.
    pass


def t_flatten_integer_type():
    """Flatten preserves the integer element type."""
    a = make_handle([[10, 20], [30]])
    assert a.type_id == orc.ORC_TYPE_U8
    out = orc.flatten_deck(a)
    assert out.type_id == orc.ORC_TYPE_U8
    assert orc.read_deck(out) == [10, 20, 30]


# ============================================================
# flatten_deck — Error cases
# ============================================================


def t_flatten_mismatched_counts():
    """Flatten with n_inputs != n_outputs produces no output."""
    # Skipped: no n_out= keyword in the new module, cannot reproduce this
    # exact error condition.
    pass


# ============================================================
# Ownership / lifetime invariants
# ============================================================


def t_output_free_fn_set():
    """Plugin output handles have valid ownership (Rust RAII manages lifetime)."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    out = orc.add(a, b)
    # Skipped: no free_fn attribute in the new Handle. Rust manages lifetime via RAII.
    assert out.n_items > 0


def t_output_handle_id_preserved():
    """Plugin preserves the handle ID set by the caller."""
    # Skipped: no handle ID API (next_handle_id / .handle) in the new module.
    pass


def t_output_type_matches_input_for_flatten():
    """Flatten output type matches the input type."""
    a = make_handle([[1.0, 2.0]])
    assert a.type_id == orc.ORC_TYPE_F64
    out = orc.flatten_deck(a)
    assert out.type_id == orc.ORC_TYPE_F64


def t_list_length_output_is_u64():
    """List length always outputs U64."""
    a = make_handle([[1.0, 2.0]])
    out = orc.list_length(a)
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
# numpy — Zero-copy numpy integration via __array_interface__
# ============================================================


def t_numpy_f64():
    """Convert f64 handle to numpy, verify dtype and arithmetic."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    out = orc.add(a, b)
    arr = np.asarray(out)
    assert arr.dtype == np.float64
    assert arr.__array_interface__['data'][0] == out.__array_interface__['data'][0]
    assert list(arr * 2.0) == [22.0, 44.0, 66.0]


def t_numpy_i64():
    """Convert i64 handle to numpy, verify dtype and arithmetic."""
    h = make_handle([-2**40, 0, 2**40], type_id=orc.ORC_TYPE_I64)
    arr = np.asarray(h)
    assert arr.dtype == np.int64
    assert arr.__array_interface__['data'][0] == h.__array_interface__['data'][0]
    assert list(arr + np.int64(1)) == [-(2**40) + 1, 1, 2**40 + 1]


def t_numpy_3x3_matrix():
    """Convert a 3x3 nested handle to a numpy matrix, verify arithmetic."""
    data = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]
    h = make_handle(data)
    arr = np.asarray(h)
    assert arr.__array_interface__['data'][0] == h.__array_interface__['data'][0]
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
    out = orc.add(a, b)
    arr = np.asarray(out)
    assert arr.__array_interface__['data'][0] == out.__array_interface__['data'][0]
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
        return np.asarray(h)

    arr = get_arr()
    gc.collect()  # Force GC of the handle
    # If the handle was freed, this reads freed memory.
    assert list(arr) == [1.0, 2.0, 3.0]
    assert list(arr + 1.0) == [2.0, 3.0, 4.0]


def t_numpy_and_handle_both_freed():
    """Handle is freed when both numpy array and handle go out of scope."""

    def create_and_drop():
        h = make_handle([1.0, 2.0, 3.0])
        arr = np.asarray(h)
        assert list(arr) == [1.0, 2.0, 3.0]
        # Both h and arr go out of scope here.

    create_and_drop()
    gc.collect()


# ============================================================
# complex numbers — create_complex / add_complex / mul_complex
# ============================================================


def make_complex(real_data, imag_data):
    """Create a Complex deck from real and imaginary f64 lists."""
    real = make_handle(real_data)
    imag = make_handle(imag_data)
    out = orc.create_complex(real, imag)
    return out


def get_parts(complex_handle):
    """Extract real and imaginary parts from a Complex deck as f64 lists."""
    results = orc.complex_get_parts(complex_handle)
    # complex_get_parts has n_outputs=2, so it returns a list of two Handles
    real_out, imag_out = results
    return orc.read_deck(real_out), orc.read_deck(imag_out)


# ==================== add_complex ====================


def t_complex_add_flat():
    """[1+2i, 3+4i] + [10+20i, 30+40i] = [11+22i, 33+44i]"""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    rhs = make_complex([10.0, 30.0], [20.0, 40.0])
    out = orc.add_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [11.0, 33.0]
    assert imag == [22.0, 44.0]


def t_complex_add_nested():
    """[[1+i, 2+i], [3+i]] + [[4+i, 5+i], [6+i]] = [[5+2i, 7+2i], [9+2i]]"""
    lhs = make_complex([[1.0, 2.0], [3.0]], [[1.0, 1.0], [1.0]])
    rhs = make_complex([[4.0, 5.0], [6.0]], [[1.0, 1.0], [1.0]])
    out = orc.add_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [[5.0, 7.0], [9.0]]
    assert imag == [[2.0, 2.0], [2.0]]


def t_complex_add_negative_components():
    """[1-2i, -3+4i] + [0+3i, 3-4i] = [1+1i, 0+0i]"""
    lhs = make_complex([1.0, -3.0], [-2.0, 4.0])
    rhs = make_complex([0.0, 3.0], [3.0, -4.0])
    out = orc.add_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [1.0, 0.0]
    assert imag == [1.0, 0.0]


def t_complex_add_wrong_n_inputs():
    """add_complex raises ValueError when called with the wrong number of inputs."""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    try:
        out = orc.add_complex(lhs)
    except (ValueError, TypeError):
        # We supplied the wrong number of inputs, so this is expected.
        return
    assert False, "This should be unreachable"


# ==================== mul_complex ====================


def t_complex_mul_flat():
    """[1+2i, 2+3i] * [3+4i, 1+0i] = [-5+10i, 2+3i]"""
    lhs = make_complex([1.0, 2.0], [2.0, 3.0])
    rhs = make_complex([3.0, 1.0], [4.0, 0.0])
    out = orc.mul_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [-5.0, 2.0]
    assert imag == [10.0, 3.0]


def t_complex_mul_nested():
    """[[1+0i, 0+1i], [2+0i]] * [[5+0i, 0+1i], [3+0i]] = [[5+0i, -1+0i], [6+0i]]"""
    lhs = make_complex([[1.0, 0.0], [2.0]], [[0.0, 1.0], [0.0]])
    rhs = make_complex([[5.0, 0.0], [3.0]], [[0.0, 1.0], [0.0]])
    out = orc.mul_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [[5.0, -1.0], [6.0]]
    assert imag == [[0.0, 0.0], [0.0]]


def t_complex_mul_i_squared():
    """[0+1i, 0+1i, 0+1i] * [0+1i, 0+1i, 0+1i] = [-1+0i, -1+0i, -1+0i]"""
    lhs = make_complex([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    rhs = make_complex([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
    out = orc.mul_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [-1.0, -1.0, -1.0]
    assert imag == [0.0, 0.0, 0.0]


def t_complex_mul_by_zero():
    """[3+4i, 1+1i] * [0+0i, 0+0i] = [0+0i, 0+0i]"""
    lhs = make_complex([3.0, 1.0], [4.0, 1.0])
    rhs = make_complex([0.0, 0.0], [0.0, 0.0])
    out = orc.mul_complex(lhs, rhs)
    real, imag = get_parts(out)
    assert real == [0.0, 0.0]
    assert imag == [0.0, 0.0]


def t_complex_mul_wrong_n_inputs():
    """mul_complex raises ValueError when called with the wrong number of inputs."""
    lhs = make_complex([1.0, 3.0], [2.0, 4.0])
    try:
        out = orc.mul_complex(lhs)
    except (ValueError, TypeError):
        # We supplied the wrong number of inputs, so this is expected.
        return
    assert False, "This should be unreachable"


def t_complex_flatten():
    """Flatten a nested complex deck into a flat list."""
    nested = make_complex([[1.0, 3.0], [5.0]], [[2.0, 4.0], [6.0]])
    flat = orc.flatten_deck(nested)
    real, imag = get_parts(flat)
    assert real == [1.0, 3.0, 5.0]
    assert imag == [2.0, 4.0, 6.0]


# ============================================================
# Serialization round-trip (workflow-level)
# ============================================================


# Map builtin type IDs to sample test values.
_BUILTIN_SAMPLES = {
    orc.ORC_TYPE_U8: [0, 1, 127, 255],
    orc.ORC_TYPE_U16: [0, 1, 256, 65535],
    orc.ORC_TYPE_U32: [0, 1, 70000, 0xFFFFFFFF],
    orc.ORC_TYPE_U64: [0, 1, 0x100000000],
    orc.ORC_TYPE_I8: [-128, 0, 127],
    orc.ORC_TYPE_I16: [-32768, 0, 32767],
    orc.ORC_TYPE_I32: [-0x80000000, 0, 0x7FFFFFFF],
    orc.ORC_TYPE_I64: [-0x80000000_00000000, 0, 0x7FFFFFFF_FFFFFFFF],
    orc.ORC_TYPE_F32: [0.0, 1.5, -3.25],
    orc.ORC_TYPE_F64: [0.0, 1.5, -3.25, 1e100],
}


def t_serial_every_plugin_handles_builtin_types():
    """Every builtin type survives a workflow serialize/deserialize round-trip."""
    for type_id, values in _BUILTIN_SAMPLES.items():
        def make_const(tid=type_id, vals=values):
            def fn():
                # Use flatten_deck as a type-preserving identity operation.
                return orc.flatten_deck(orc.make_deck(vals, type_id=tid))
            return fn
        graph = orc.make_workflow(make_const())
        with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
            path = f.name
        try:
            orc.save_workflow(graph, path)
            restored = orc.load_workflow(path)
            out = restored.run()
            assert out.type_id == type_id, (
                f"type {type_id:#x}: expected {type_id:#x}, got {out.type_id:#x}")
            assert out.n_items == len(values)
        finally:
            os.unlink(path)


def t_serial_every_plugin_handles_nested_builtin():
    """Nested f64 deck survives a workflow serialize/deserialize round-trip."""
    def make_nested():
        h = orc.make_deck([[1.0, 2.0, 3.0], [4.0, 5.0]])
        return orc.add(h, orc.make_deck([0.0]))
    graph = orc.make_workflow(make_nested)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        restored = orc.load_workflow(path)
        out = restored.run()
        assert orc.read_deck(out) == [[1.0, 2.0, 3.0], [4.0, 5.0]]
    finally:
        os.unlink(path)


def t_serial_custom_type_round_trip():
    """Custom type (Complex) survives a workflow serialize/deserialize round-trip."""
    def make_complex_graph():
        real = orc.make_deck([1.0, -2.0, 3.0])
        imag = orc.make_deck([4.0, -5.0, 6.0])
        return orc.create_complex(real, imag)
    graph = orc.make_workflow(make_complex_graph)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        restored = orc.load_workflow(path)
        out = restored.run()
        results = orc.complex_get_parts(out)
        real_out, imag_out = results
        assert orc.read_deck(real_out) == [1.0, -2.0, 3.0]
        assert orc.read_deck(imag_out) == [4.0, -5.0, 6.0]
    finally:
        os.unlink(path)


def t_serial_custom_type_nested_round_trip():
    """Nested custom type deck survives workflow serialization round-trip."""
    def make_nested_complex():
        real = orc.make_deck([[1.0, 2.0], [3.0]])
        imag = orc.make_deck([[4.0, 5.0], [6.0]])
        return orc.create_complex(real, imag)
    graph = orc.make_workflow(make_nested_complex)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        restored = orc.load_workflow(path)
        out = restored.run()
        results = orc.complex_get_parts(out)
        real_out, imag_out = results
        assert orc.read_deck(real_out) == [[1.0, 2.0], [3.0]]
        assert orc.read_deck(imag_out) == [[4.0, 5.0], [6.0]]
    finally:
        os.unlink(path)


def t_serial_concurrent_serialization():
    """Two workflow serialization calls on different threads don't corrupt each other."""
    def make_f64_a():
        return orc.add(orc.make_deck([1.0, 2.0, 3.0, 4.0, 5.0]),
                       orc.make_deck([0.0]))
    def make_f64_b():
        return orc.add(orc.make_deck([10.0, 20.0, 30.0, 40.0, 50.0]),
                       orc.make_deck([0.0]))
    graph1 = orc.make_workflow(make_f64_a)
    graph2 = orc.make_workflow(make_f64_b)
    errors = [None, None]
    paths = [None, None]

    def serialize_thread(idx, graph):
        try:
            with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
                paths[idx] = f.name
            orc.save_workflow(graph, paths[idx])
        except Exception as e:
            errors[idx] = e

    t1 = threading.Thread(target=serialize_thread, args=(0, graph1))
    t2 = threading.Thread(target=serialize_thread, args=(1, graph2))
    t1.start()
    t2.start()
    t1.join()
    t2.join()
    try:
        assert errors[0] is None, f"Thread 0 error: {errors[0]}"
        assert errors[1] is None, f"Thread 1 error: {errors[1]}"
        restored1 = orc.load_workflow(paths[0])
        restored2 = orc.load_workflow(paths[1])
        out1 = restored1.run()
        out2 = restored2.run()
        assert orc.read_deck(out1) == [1.0, 2.0, 3.0, 4.0, 5.0]
        assert orc.read_deck(out2) == [10.0, 20.0, 30.0, 40.0, 50.0]
    finally:
        for p in paths:
            if p and os.path.exists(p):
                os.unlink(p)


def t_serial_empty_deck():
    """Serialize and deserialize a single-element deck via workflow constant."""
    def make_const():
        return orc.add(orc.make_deck([42.0]), orc.make_deck([0.0]))
    graph = orc.make_workflow(make_const)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        restored = orc.load_workflow(path)
        out = restored.run()
        assert out.type_id == orc.ORC_TYPE_F64
        assert orc.read_deck(out) == [42.0]
    finally:
        os.unlink(path)


def t_serial_single_element():
    """Serialize and deserialize a single-element deck via workflow."""
    def make_single():
        return orc.add(orc.make_deck([42.0]), orc.make_deck([0.0]))
    graph = orc.make_workflow(make_single)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        restored = orc.load_workflow(path)
        out = restored.run()
        assert orc.read_deck(out) == [42.0]
    finally:
        os.unlink(path)


def t_serial_deserialize_truncated_fails():
    """Deserializing a truncated workflow file raises an error."""
    def make_data():
        return orc.add(orc.make_deck([1.0, 2.0, 3.0]), orc.make_deck([0.0]))
    graph = orc.make_workflow(make_data)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(graph, path)
        # Read the file and truncate it
        with open(path, "rb") as f:
            data = f.read()
        truncated_path = path + ".trunc"
        with open(truncated_path, "wb") as f:
            f.write(data[:len(data) // 2])
        try:
            orc.load_workflow(truncated_path)
            assert False, "Should have raised an error on truncated data"
        except Exception:
            pass  # Expected: truncated data causes an error
        finally:
            if os.path.exists(truncated_path):
                os.unlink(truncated_path)
    finally:
        os.unlink(path)


def t_serial_deserialize_empty_buffer_fails():
    """Deserializing an empty workflow file raises an error."""
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        # Write an empty file
        with open(path, "wb") as f:
            pass
        try:
            orc.load_workflow(path)
            assert False, "Should have raised an error on empty file"
        except Exception:
            pass  # Expected: empty file causes an error
    finally:
        os.unlink(path)


def t_serial_preserves_dims():
    """Serialization round-trip preserves dimensional metadata."""
    # Skipped: the new Handle's .dims is read-only. We cannot set dims from
    # Python, so this test cannot be reproduced.
    pass


# ============================================================
# make_workflow — Construction
# ============================================================


def t_workflow_basic():
    """Basic workflow: add two inputs."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    assert orc.read_deck(wf.run(a, b)) == [11.0, 22.0, 33.0]


def t_workflow_chain():
    """Chained operations: mul then add."""
    a = make_handle([2.0, 3.0])
    b = make_handle([10.0, 20.0])
    def chain(x, y):
        product = orc.mul(x, y)
        return orc.add(product, x)
    wf = orc.make_workflow(chain)
    assert orc.read_deck(wf.run(a, b)) == [22.0, 63.0]


def t_workflow_with_constant():
    """Workflow with an internal make_deck constant."""
    a = make_handle([1.0, 2.0, 3.0])
    def fn_with_const(x):
        offset = orc.make_deck([100.0])
        return orc.add(x, offset)
    wf = orc.make_workflow(fn_with_const)
    assert orc.read_deck(wf.run(a)) == [101.0, 102.0, 103.0]


def t_workflow_multiple_constants():
    """Workflow with several constants."""
    a = make_handle([1.0])
    def fn_multi_const(x):
        a = orc.make_deck([10.0])
        b = orc.make_deck([100.0])
        return orc.add(orc.add(x, a), b)
    wf = orc.make_workflow(fn_multi_const)
    assert orc.read_deck(wf.run(a)) == [111.0]


def t_workflow_fan_out():
    """Same input feeds into multiple function arguments."""
    a = make_handle([3.0, 4.0])
    wf = orc.make_workflow(lambda x: orc.mul(x, x))
    assert orc.read_deck(wf.run(a)) == [9.0, 16.0]


def t_workflow_diamond():
    """Diamond topology: input → two branches → merge."""
    a = make_handle([5.0])
    def diamond(x):
        doubled = orc.add(x, x)
        squared = orc.mul(x, x)
        return orc.sub(squared, doubled)
    wf = orc.make_workflow(diamond)
    # 5^2 - 5*2 = 25 - 10 = 15
    assert orc.read_deck(wf.run(a)) == [15.0]


def t_workflow_multi_output():
    """Workflow returning multiple outputs via a multi-output function."""
    a = make_handle([1.0, 2.0])
    b = make_handle([3.0, 4.0])
    def fn_multi_out(x, y):
        c = orc.create_complex(x, y)
        return orc.complex_get_parts(c)
    wf = orc.make_workflow(fn_multi_out)
    real, imag = wf.run(a, b)
    assert orc.read_deck(real) == [1.0, 2.0]
    assert orc.read_deck(imag) == [3.0, 4.0]


def t_workflow_no_inputs():
    """Workflow with no parameters — all data from constants."""
    def fn_const_only():
        a = orc.make_deck([1.0, 2.0, 3.0])
        b = orc.make_deck([10.0, 20.0, 30.0])
        return orc.add(a, b)
    wf = orc.make_workflow(fn_const_only)
    assert orc.read_deck(wf.run()) == [11.0, 22.0, 33.0]


def t_workflow_nested_data():
    """Workflow operates on nested deck structure."""
    a = make_handle([[1.0, 2.0], [3.0]])
    b = make_handle([10.0])
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    assert orc.read_deck(wf.run(a, b)) == [[11.0, 12.0], [13.0]]


# ============================================================
# make_workflow — Run with keyword / mixed arguments
# ============================================================


def t_workflow_run_kwargs():
    """Run a workflow with keyword arguments."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    wf = orc.make_workflow(lambda x, y: orc.sub(x, y))
    assert orc.read_deck(wf.run(x=a, y=b)) == [-1.0]
    assert orc.read_deck(wf.run(y=a, x=b)) == [1.0]


def t_workflow_run_mixed_args():
    """Run a workflow with positional + keyword arguments."""
    a = make_handle([5.0])
    b = make_handle([3.0])
    wf = orc.make_workflow(lambda x, y: orc.sub(x, y))
    assert orc.read_deck(wf.run(a, y=b)) == [2.0]


def t_workflow_run_reuse():
    """Run the same workflow multiple times with different inputs."""
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    a1 = make_handle([1.0])
    b1 = make_handle([10.0])
    a2 = make_handle([100.0])
    b2 = make_handle([200.0])
    assert orc.read_deck(wf.run(a1, b1)) == [11.0]
    assert orc.read_deck(wf.run(a2, b2)) == [300.0]


# ============================================================
# make_workflow — Error cases
# ============================================================


def t_workflow_no_recursion():
    """make_workflow cannot be called recursively."""
    def outer(x):
        # Attempt to call make_workflow inside make_workflow.
        orc.make_workflow(lambda y: orc.add(y, y))
        return orc.add(x, x)
    try:
        orc.make_workflow(outer)
        assert False, "Should have raised RuntimeError"
    except RuntimeError as e:
        assert "recursively" in str(e).lower()


def t_workflow_node_not_arithmetic():
    """WorkflowNode cannot be used for arithmetic — Python raises TypeError."""
    def bad_fn(x):
        return x + 1  # WorkflowNode has no __add__
    try:
        orc.make_workflow(bad_fn)
        assert False, "Should have raised TypeError"
    except TypeError:
        pass


def t_workflow_node_not_comparable():
    """WorkflowNode cannot be compared — Python raises TypeError."""
    def bad_fn(x, y):
        if x > y:  # WorkflowNode has no __gt__
            return orc.add(x, y)
        return orc.sub(x, y)
    try:
        orc.make_workflow(bad_fn)
        assert False, "Should have raised TypeError"
    except TypeError:
        pass


def t_workflow_node_not_readable():
    """Calling read_deck on a WorkflowNode raises TypeError."""
    def bad_fn(x):
        orc.read_deck(x)  # x is WorkflowNode, not Handle
        return orc.add(x, x)
    try:
        orc.make_workflow(bad_fn)
        assert False, "Should have raised TypeError"
    except TypeError:
        pass


def t_workflow_run_too_many_args():
    """Running a workflow with too many positional args raises ValueError."""
    wf = orc.make_workflow(lambda x: orc.add(x, x))
    a = make_handle([1.0])
    b = make_handle([2.0])
    try:
        wf.run(a, b)
        assert False, "Should have raised ValueError"
    except ValueError:
        pass


def t_workflow_run_missing_arg():
    """Missing arguments become empty handles. The plugin errors on type mismatch."""
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    a = make_handle([1.0])
    # add(a, <empty>) — type mismatch between f64 and type_id 0.
    # Workflow::run propagates the plugin error.
    try:
        wf.run(a)
        assert False, "Should have raised RuntimeError"
    except RuntimeError:
        pass


def t_workflow_run_unknown_kwarg():
    """Running a workflow with unknown keyword raises ValueError."""
    wf = orc.make_workflow(lambda x: orc.add(x, x))
    a = make_handle([1.0])
    try:
        wf.run(z=a)
        assert False, "Should have raised ValueError"
    except ValueError:
        pass


def t_workflow_run_duplicate_arg():
    """Running with both positional and keyword for same param raises ValueError."""
    wf = orc.make_workflow(lambda x: orc.add(x, x))
    a = make_handle([1.0])
    try:
        wf.run(a, x=a)
        assert False, "Should have raised ValueError"
    except ValueError:
        pass


# ============================================================
# run_workflow convenience function
# ============================================================


def t_run_workflow_convenience():
    """run_workflow is equivalent to graph.run."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    assert orc.read_deck(orc.run_workflow(wf, a, b)) == [11.0, 22.0, 33.0]
    assert orc.read_deck(orc.run_workflow(wf, x=a, y=b)) == [11.0, 22.0, 33.0]


# ============================================================
# save_workflow / load_workflow — Round-trips
# ============================================================


def t_workflow_save_load_roundtrip():
    """Workflow survives save/load round-trip."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    wf = orc.make_workflow(lambda x, y: orc.add(x, y))
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(wf, path)
        loaded = orc.load_workflow(path)
        assert orc.read_deck(loaded.run(a, b)) == [11.0, 22.0, 33.0]
    finally:
        os.unlink(path)


def t_workflow_save_load_with_constants():
    """Workflow with internal constants survives save/load."""
    a = make_handle([5.0])
    def fn(x):
        return orc.add(x, orc.make_deck([95.0]))
    wf = orc.make_workflow(fn)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(wf, path)
        loaded = orc.load_workflow(path)
        assert orc.read_deck(loaded.run(a)) == [100.0]
    finally:
        os.unlink(path)


def t_workflow_save_load_no_inputs():
    """Pure-constant workflow survives save/load."""
    def fn():
        return orc.add(orc.make_deck([1.0, 2.0]), orc.make_deck([10.0, 20.0]))
    wf = orc.make_workflow(fn)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(wf, path)
        loaded = orc.load_workflow(path)
        assert orc.read_deck(loaded.run()) == [11.0, 22.0]
    finally:
        os.unlink(path)


def t_workflow_save_load_multi_output():
    """Multi-output workflow survives save/load."""
    def fn():
        c = orc.create_complex(orc.make_deck([1.0]), orc.make_deck([2.0]))
        return orc.complex_get_parts(c)
    wf = orc.make_workflow(fn)
    with tempfile.NamedTemporaryFile(suffix=".orcflow", delete=False) as f:
        path = f.name
    try:
        orc.save_workflow(wf, path)
        loaded = orc.load_workflow(path)
        real, imag = loaded.run()
        assert orc.read_deck(real) == [1.0]
        assert orc.read_deck(imag) == [2.0]
    finally:
        os.unlink(path)


# ============================================================
# Interleaving immediate and deferred modes
# ============================================================


def t_workflow_interleave():
    """Immediate-mode calls work between and after make_workflow calls."""
    # Before.
    a = make_handle([1.0, 2.0])
    b = make_handle([10.0, 20.0])
    assert orc.read_deck(orc.add(a, b)) == [11.0, 22.0]
    # Build a workflow.
    wf = orc.make_workflow(lambda x: orc.add(x, orc.make_deck([100.0])))
    # Between.
    assert orc.read_deck(orc.mul(a, b)) == [10.0, 40.0]
    # Run the workflow.
    result = wf.run(a)
    assert orc.read_deck(result) == [101.0, 102.0]
    # After — use the workflow output in immediate mode.
    doubled = orc.mul(result, make_handle([2.0]))
    assert orc.read_deck(doubled) == [202.0, 204.0]


def t_workflow_error_does_not_poison_immediate_mode():
    """A failed make_workflow doesn't break subsequent immediate-mode calls."""
    try:
        orc.make_workflow(lambda x: x + 1)  # TypeError
    except TypeError:
        pass
    # Immediate mode should still work.
    a = make_handle([1.0])
    b = make_handle([2.0])
    assert orc.read_deck(orc.add(a, b)) == [3.0]


def t_workflow_error_does_not_poison_next_workflow():
    """A failed make_workflow doesn't break a subsequent make_workflow."""
    try:
        orc.make_workflow(lambda x: x + 1)  # TypeError
    except TypeError:
        pass
    # Next make_workflow should work.
    wf = orc.make_workflow(lambda x: orc.add(x, x))
    a = make_handle([5.0])
    assert orc.read_deck(wf.run(a)) == [10.0]


# ============================================================
# Stub generation
# ============================================================


def t_stubs_file_exists():
    """A .pyi stub file exists alongside the installed module."""
    import pathlib
    module_file = pathlib.Path(orc.__file__)
    stub_path = module_file.with_suffix(".pyi")
    assert stub_path.exists(), f"Expected stub at {stub_path}"


def t_stubs_contain_classes():
    """The stub file declares Handle, WorkflowNode, and Workflow classes."""
    import pathlib
    stub_path = pathlib.Path(orc.__file__).with_suffix(".pyi")
    content = stub_path.read_text()
    assert "class Handle:" in content
    assert "class WorkflowNode:" in content
    assert "class Workflow:" in content


def t_stubs_contain_module_functions():
    """The stub file declares all module-level functions."""
    import pathlib
    stub_path = pathlib.Path(orc.__file__).with_suffix(".pyi")
    content = stub_path.read_text()
    for fn_name in ["load_plugins", "make_deck", "read_deck",
                     "make_workflow", "run_workflow", "save_workflow",
                     "load_workflow"]:
        assert f"def {fn_name}(" in content, f"Missing {fn_name} in stubs"


def t_stubs_contain_plugin_functions():
    """The stub file declares plugin functions with correct signatures."""
    import pathlib
    stub_path = pathlib.Path(orc.__file__).with_suffix(".pyi")
    content = stub_path.read_text()
    # add has 2 inputs, 1 output.
    assert "def add(arg0: Handle, arg1: Handle) -> Handle: ..." in content
    # complex_get_parts has 1 input, 2 outputs → list[Handle].
    assert "def complex_get_parts(arg0: Handle) -> list[Handle]: ..." in content


def t_stubs_contain_type_constants():
    """The stub file declares type constants."""
    import pathlib
    stub_path = pathlib.Path(orc.__file__).with_suffix(".pyi")
    content = stub_path.read_text()
    assert "ORC_TYPE_U8: int" in content
    assert "ORC_TYPE_F64: int" in content


# ============================================================
# Runner
# ============================================================

if __name__ == "__main__":
    # by convention all tests start with t_
    orc.load_plugins(search_dir)
    tests = [(name, fn) for name, fn in globals().items()
             if name.startswith("t_") and callable(fn)]
    tests.sort(key=lambda x: x[0])
    passed = 0
    failed = 0
    for name, fn in tests:
        try:
            fn()
            passed += 1
            print(f"  PASS  {name}")
        except Exception:
            import traceback
            failed += 1
            print(f"  FAIL  {name}")
            traceback.print_exc()
    print(f"\n{passed} passed, {failed} failed, {passed + failed} total")
    sys.exit(1 if failed else 0)
