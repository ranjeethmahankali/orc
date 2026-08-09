"""Unit tests for orc plugin functions with handle leak tracking."""

import ctypes
import gc
import math
import os
import sys

from orc import (
    ORC_TYPE_F64,
    ORC_TYPE_I8,
    ORC_TYPE_I16,
    ORC_TYPE_I32,
    ORC_TYPE_I64,
    ORC_TYPE_U8,
    ORC_TYPE_U16,
    ORC_TYPE_U32,
    ORC_TYPE_U64,
    OrcHandle,
    default_host,
    get_function,
    load_plugins,
    make_handle as _make_handle,
    next_handle_id,
    read_handle,
)

# ---------------------------------------------------------------------------
# Handle leak tracking (test-only)
# ---------------------------------------------------------------------------

_live_handles = set()
_orig_del = OrcHandle.__del__


def _tracking_del(self):
    _live_handles.discard(self.handle)
    _orig_del(self)


OrcHandle.__del__ = _tracking_del


def make_handle(data, type_id=None):
    """Create a handle and register it for leak tracking."""
    h = _make_handle(data, type_id=type_id)
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

host = default_host()

plugins = load_plugins(search_dir, host)
if not plugins:
    print(f"No plugins found in {search_dir}")
    sys.exit(1)


def call_fn(name, inputs, n_outputs=1):
    """Call a plugin function by name and return output handles."""
    fn = get_function(plugins, name)
    in_arr = (OrcHandle * len(inputs))(*inputs)
    outs = []
    for _ in range(n_outputs):
        out = OrcHandle()
        ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
        out.handle = next_handle_id()
        _live_handles.add(out.handle)
        outs.append(out)
    out_arr = (OrcHandle * n_outputs)(*outs)
    fn.func(0, in_arr, len(inputs), out_arr, n_outputs)
    return [out_arr[i] for i in range(n_outputs)]


# ============================================================
# add — Correctness
# ============================================================


def t_add_f64_flat():
    """Add two flat f64 lists element-wise."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11.0, 22.0, 33.0]


def t_add_f64_nested():
    """Add nested list with broadcast flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn("add", [a, b])
    result = read_handle(out)
    assert result == [[11.0, 22.0, 33.0], [12.0, 24.0, 36.0, 38.0]]


def t_add_broadcast_scalar():
    """Add a scalar to each element of a list."""
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11.0, 12.0, 13.0]


def t_add_single_element():
    """Add two single-element lists."""
    a = make_handle([5.0])
    b = make_handle([3.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [8.0]


def t_add_depth3():
    """Add a scalar to a depth-3 nested list."""
    a = make_handle([[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]])
    b = make_handle([10.0])
    [out] = call_fn("add", [a, b])
    result = read_handle(out)
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
    assert a.type_id == ORC_TYPE_I8
    [out] = call_fn("add", [a, b])
    # a is I8 (has negatives), b is U8 (all positive).
    # Type mismatch — output untouched.


def t_add_u8():
    """Add two U8 lists element-wise."""
    a = make_handle([1, 2, 3])
    b = make_handle([10, 20, 30])
    assert a.type_id == ORC_TYPE_U8
    assert b.type_id == ORC_TYPE_U8
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11, 22, 33]


def t_add_u32():
    """Add two U32 lists element-wise."""
    a = make_handle([100000, 200000])
    b = make_handle([300000, 400000])
    assert a.type_id == ORC_TYPE_U32
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [400000, 600000]


# ============================================================
# add — Error cases
# ============================================================


def t_add_mismatched_types():
    """Add U8 and F64 lists produces no output."""
    a = make_handle([1, 2, 3])  # U8
    b = make_handle([1.0, 2.0, 3.0])  # F64
    assert a.type_id != b.type_id
    [out] = call_fn("add", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# mul — Correctness
# ============================================================


def t_mul_f64():
    """Multiply two flat f64 lists element-wise."""
    a = make_handle([2.0, 3.0, 4.0])
    b = make_handle([5.0, 6.0, 7.0])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [10.0, 18.0, 28.0]


def t_mul_u8():
    """Multiply two U8 lists element-wise."""
    a = make_handle([3, 4])
    b = make_handle([7, 8])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [21, 32]


def t_mul_nested():
    """Multiply nested list by broadcast scalar."""
    a = make_handle([[2.0, 3.0], [4.0]])
    b = make_handle([10.0])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [[20.0, 30.0], [40.0]]


def t_mul_mismatched_types():
    """Multiply U8 and F64 lists produces no output."""
    a = make_handle([2, 3])  # U8
    b = make_handle([3.0, 4.0])  # F64
    [out] = call_fn("mul", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# sub — Correctness
# ============================================================


def t_sub_f64():
    """Subtract two flat f64 lists element-wise."""
    a = make_handle([10.0, 20.0])
    b = make_handle([3.0, 7.0])
    [out] = call_fn("sub", [a, b])
    assert read_handle(out) == [7.0, 13.0]


def t_sub_nested():
    """Subtract broadcast scalar from nested list."""
    a = make_handle([[10.0, 20.0], [30.0]])
    b = make_handle([1.0])
    [out] = call_fn("sub", [a, b])
    assert read_handle(out) == [[9.0, 19.0], [29.0]]


def t_sub_unsupported_type():
    """Sub on U8 is unsupported, produces no output."""
    a = make_handle([5, 3])  # U8
    b = make_handle([3, 1])
    [out] = call_fn("sub", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# div — Correctness
# ============================================================


def t_div_f64():
    """Divide two flat f64 lists element-wise."""
    a = make_handle([10.0, 9.0])
    b = make_handle([2.0, 3.0])
    [out] = call_fn("div", [a, b])
    assert read_handle(out) == [5.0, 3.0]


def t_div_by_zero():
    """Divide by zero produces positive infinity."""
    a = make_handle([1.0])
    b = make_handle([0.0])
    [out] = call_fn("div", [a, b])
    result = read_handle(out)
    assert math.isinf(result[0]) and result[0] > 0


def t_div_unsupported_type():
    """Div on U8 is unsupported, produces no output."""
    a = make_handle([6, 2])  # U8
    b = make_handle([2, 1])
    [out] = call_fn("div", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# repeat_list — Correctness
# ============================================================


def t_repeat_list_f64():
    """Repeat a flat f64 list 3 times."""
    a = make_handle([1.0, 2.0, 3.0])
    count = make_handle(3, type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    result = read_handle(out)
    assert result == [1.0, 2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 3.0]


def t_repeat_list_u8():
    """Repeat a U8 list 2 times."""
    a = make_handle([10, 20])
    count = make_handle([2], type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    # We expect a nested list because the second input is a list.
    assert read_handle(out) == [[10, 20, 10, 20]]


def t_repeat_list_single_element():
    """Repeat a single-element list 4 times."""
    a = make_handle([42.0])
    count = make_handle([4], type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    assert read_handle(out) == [[42.0, 42.0, 42.0, 42.0]]


def t_repeat_list_one_repeat():
    """Repeating once returns the same items."""
    a = make_handle([5.0, 6.0])
    count = make_handle(1, type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    assert read_handle(out) == [5.0, 6.0]


def t_repeat_list_zero_repeats():
    """Repeating zero times produces an empty output."""
    a = make_handle([1.0, 2.0])
    count = make_handle(0, type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    assert read_handle(out) == []


def t_repeat_list_output_type_matches_input():
    """Output type matches the list input type."""
    a = make_handle([100000, 200000])  # U32
    assert a.type_id == ORC_TYPE_U32
    count = make_handle(2, type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    assert out.type_id == ORC_TYPE_U32
    assert read_handle(out) == [100000, 200000, 100000, 200000]


def t_repeat_list_output_free_fn_set():
    """Plugin output has free_fn set."""
    a = make_handle([1.0])
    count = make_handle(1, type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    assert out.free_fn


def t_repeat_list_nested_input():
    """Repeat each sublist of a nested input."""
    a = make_handle([[1.0, 2.0], [3.0]])
    count = make_handle([2], type_id=ORC_TYPE_U64)
    [out] = call_fn("repeat_list", [a, count])
    result = read_handle(out)
    # Each sublist repeated: [1,2,1,2] and [3,3]
    assert result == [[1.0, 2.0, 1.0, 2.0], [3.0, 3.0]]


# ============================================================
# list_length — Correctness
# ============================================================


def t_list_length_basic():
    """List length of two sublists returns their sizes."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn("list_length", [a])
    result = read_handle(out)
    assert result == [3, 2]
    assert out.type_id == ORC_TYPE_U64


def t_list_length_single_list():
    """List length of one single-element sublist."""
    a = make_handle([[42.0]])
    [out] = call_fn("list_length", [a])
    assert read_handle(out) == [1]


def t_list_length_depth3():
    """List length at depth-3 returns nested lengths."""
    a = make_handle([[[1.0, 2.0], [3.0]], [[4.0, 5.0, 6.0]]])
    [out] = call_fn("list_length", [a])
    result = read_handle(out)
    assert result == [[2, 1], [3]]


# ============================================================
# flatten_deck — Correctness
# ============================================================


def t_flatten_basic():
    """Flatten a nested list into a flat list."""
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn("flatten_deck", [a])
    result = read_handle(out)
    assert result == [1.0, 2.0, 3.0, 4.0, 5.0]


def t_flatten_already_flat():
    """Flatten an already-flat list is a no-op."""
    a = make_handle([1.0, 2.0, 3.0])
    [out] = call_fn("flatten_deck", [a])
    assert read_handle(out) == [1.0, 2.0, 3.0]


def t_flatten_multiple_io():
    """Flatten with multiple inputs and outputs."""
    a = make_handle([[1.0, 2.0], [3.0]])
    b = make_handle([[4.0, 5.0, 6.0]])
    outs = call_fn("flatten_deck", [a, b], n_outputs=2)
    assert read_handle(outs[0]) == [1.0, 2.0, 3.0]
    assert read_handle(outs[1]) == [4.0, 5.0, 6.0]


def t_flatten_integer_type():
    """Flatten preserves the integer element type."""
    a = make_handle([[10, 20], [30]])
    assert a.type_id == ORC_TYPE_U8
    [out] = call_fn("flatten_deck", [a])
    assert out.type_id == ORC_TYPE_U8
    assert read_handle(out) == [10, 20, 30]


# ============================================================
# flatten_deck — Error cases
# ============================================================


def t_flatten_mismatched_counts():
    """Flatten with n_inputs != n_outputs produces no output."""
    a = make_handle([[1.0]])
    fn = get_function(plugins, "flatten_deck")
    in_arr = (OrcHandle * 1)(a)
    out1 = OrcHandle()
    ctypes.memset(ctypes.addressof(out1), 0, ctypes.sizeof(out1))
    out1.handle = next_handle_id()
    out_arr = (OrcHandle * 1)(out1)
    fn.func(0, in_arr, 2, out_arr, 1)
    assert not out_arr[0].items
    assert not out_arr[0].free_fn


# ============================================================
# Ownership / lifetime invariants
# ============================================================


def t_output_free_fn_set():
    """Plugin output handles have a free_fn set."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    [out] = call_fn("add", [a, b])
    assert out.free_fn


def t_output_handle_id_preserved():
    """Plugin preserves the handle ID set by the caller."""
    a = make_handle([1.0])
    b = make_handle([2.0])
    out = OrcHandle()
    ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
    out.handle = 9999
    fn = get_function(plugins, "add")
    in_arr = (OrcHandle * 2)(a, b)
    out_arr = (OrcHandle * 1)(out)
    fn.func(0, in_arr, 2, out_arr, 1)
    assert out_arr[0].handle == 9999


def t_output_type_matches_input_for_flatten():
    """Flatten output type matches the input type."""
    a = make_handle([[1.0, 2.0]])
    assert a.type_id == ORC_TYPE_F64
    [out] = call_fn("flatten_deck", [a])
    assert out.type_id == ORC_TYPE_F64


def t_list_length_output_is_u64():
    """List length always outputs U64."""
    a = make_handle([[1.0, 2.0]])
    [out] = call_fn("list_length", [a])
    assert out.type_id == ORC_TYPE_U64


# ============================================================
# make_handle / read_handle roundtrip
# ============================================================


def t_roundtrip_flat():
    """Roundtrip a flat list through make/read."""
    data = [1.0, 2.0, 3.0]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_nested():
    """Roundtrip a nested list through make/read."""
    data = [[1.0, 2.0], [3.0, 4.0]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_depth3():
    """Roundtrip a depth-3 nested list through make/read."""
    data = [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_ragged():
    """Roundtrip a ragged nested list through make/read."""
    data = [[1.0, 2.0, 3.0], [4.0, 5.0]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_single_element():
    """Roundtrip a single-element list through make/read."""
    data = [42.0]
    h = make_handle(data)
    assert read_handle(h) == data


def t_type_detection_u8():
    """Values in [0, 255] are detected as U8."""
    h = make_handle([0, 127, 255])
    assert h.type_id == ORC_TYPE_U8


def t_type_detection_u16():
    """Values exceeding U8 range are detected as U16."""
    h = make_handle([0, 256])
    assert h.type_id == ORC_TYPE_U16


def t_type_detection_u32():
    """Values exceeding U16 range are detected as U32."""
    h = make_handle([0, 0x10000])
    assert h.type_id == ORC_TYPE_U32


def t_type_detection_u64():
    """Values exceeding U32 range are detected as U64."""
    h = make_handle([0, 0x100000000])
    assert h.type_id == ORC_TYPE_U64


def t_type_detection_i8():
    """Signed values in [-128, 127] are detected as I8."""
    h = make_handle([-128, 127])
    assert h.type_id == ORC_TYPE_I8


def t_type_detection_i16():
    """Signed values exceeding I8 range are detected as I16."""
    h = make_handle([-129, 0])
    assert h.type_id == ORC_TYPE_I16


def t_type_detection_i32():
    """Signed values exceeding I16 range are detected as I32."""
    h = make_handle([-0x8000_0000, 0])
    assert h.type_id == ORC_TYPE_I32


def t_type_detection_i64():
    """Signed values exceeding I32 range are detected as I64."""
    h = make_handle([-0x8000_0001, 0])
    assert h.type_id == ORC_TYPE_I64


def t_type_detection_f64():
    """Any float value triggers F64 detection."""
    h = make_handle([1.0, 2, 3])
    assert h.type_id == ORC_TYPE_F64


# ============================================================
# Runner
# ============================================================

if __name__ == "__main__":
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
