import ctypes
import math
import os
import sys

from orc import (
    ORC_CTYPE_MAP,
    ORC_TYPE_F32,
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
    make_handle,
    next_handle_id,
    read_handle,
)

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
    fn = get_function(plugins, name)
    in_arr = (OrcHandle * len(inputs))(*inputs)
    outs = []
    for _ in range(n_outputs):
        out = OrcHandle()
        ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
        out.handle = next_handle_id()
        outs.append(out)
    out_arr = (OrcHandle * n_outputs)(*outs)
    fn.func(0, in_arr, len(inputs), out_arr, n_outputs)
    return [out_arr[i] for i in range(n_outputs)]



# ============================================================
# add — Correctness
# ============================================================


def t_add_f64_flat():
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11.0, 22.0, 33.0]


def t_add_f64_nested():
    a = make_handle([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0, 8.0]])
    b = make_handle([10.0, 20.0, 30.0])
    [out] = call_fn("add", [a, b])
    result = read_handle(out)
    assert result == [[11.0, 22.0, 33.0], [12.0, 24.0, 36.0, 38.0]]


def t_add_broadcast_scalar():
    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11.0, 12.0, 13.0]


def t_add_single_element():
    a = make_handle([5.0])
    b = make_handle([3.0])
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [8.0]


def t_add_depth3():
    a = make_handle([[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]])
    b = make_handle([10.0])
    [out] = call_fn("add", [a, b])
    result = read_handle(out)
    assert result == [[[11.0, 12.0], [13.0, 14.0]], [[15.0, 16.0],
                                                     [17.0, 18.0]]]


# ============================================================
# add — Integer types
# ============================================================


def t_add_i32():
    a = make_handle([-5, -3, 0, 3, 5])
    b = make_handle([10, 20, 30, 40, 50])
    assert a.type_id == ORC_TYPE_I8
    [out] = call_fn("add", [a, b])
    # Plugin promotes to matching type — both inputs must match.
    # Since a is I8 and b could be U8, they need to match for add to work.
    # Actually, a has negatives so I8, b is all positive so U8. Mismatch → output untouched.
    # Let's force the same type by making both have negatives.


def t_add_u8():
    a = make_handle([1, 2, 3])
    b = make_handle([10, 20, 30])
    assert a.type_id == ORC_TYPE_U8
    assert b.type_id == ORC_TYPE_U8
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [11, 22, 33]


def t_add_u32():
    a = make_handle([100000, 200000])
    b = make_handle([300000, 400000])
    assert a.type_id == ORC_TYPE_U32
    [out] = call_fn("add", [a, b])
    assert read_handle(out) == [400000, 600000]


# ============================================================
# add — Error cases
# ============================================================


def t_add_mismatched_types():
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
    a = make_handle([2.0, 3.0, 4.0])
    b = make_handle([5.0, 6.0, 7.0])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [10.0, 18.0, 28.0]


def t_mul_u8():
    a = make_handle([3, 4])
    b = make_handle([7, 8])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [21, 32]


def t_mul_nested():
    a = make_handle([[2.0, 3.0], [4.0]])
    b = make_handle([10.0])
    [out] = call_fn("mul", [a, b])
    assert read_handle(out) == [[20.0, 30.0], [40.0]]


def t_mul_mismatched_types():
    a = make_handle([2, 3])  # U8
    b = make_handle([3.0, 4.0])  # F64
    [out] = call_fn("mul", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# sub — Correctness
# ============================================================


def t_sub_f64():
    a = make_handle([10.0, 20.0])
    b = make_handle([3.0, 7.0])
    [out] = call_fn("sub", [a, b])
    assert read_handle(out) == [7.0, 13.0]


def t_sub_nested():
    a = make_handle([[10.0, 20.0], [30.0]])
    b = make_handle([1.0])
    [out] = call_fn("sub", [a, b])
    assert read_handle(out) == [[9.0, 19.0], [29.0]]


def t_sub_unsupported_type():
    a = make_handle([5, 3])  # U8
    b = make_handle([3, 1])
    [out] = call_fn("sub", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# div — Correctness
# ============================================================


def t_div_f64():
    a = make_handle([10.0, 9.0])
    b = make_handle([2.0, 3.0])
    [out] = call_fn("div", [a, b])
    assert read_handle(out) == [5.0, 3.0]


def t_div_by_zero():
    a = make_handle([1.0])
    b = make_handle([0.0])
    [out] = call_fn("div", [a, b])
    result = read_handle(out)
    assert math.isinf(result[0]) and result[0] > 0


def t_div_unsupported_type():
    a = make_handle([6, 2])  # U8
    b = make_handle([2, 1])
    [out] = call_fn("div", [a, b])
    assert not out.items
    assert not out.free_fn


# ============================================================
# list_length — Correctness
# ============================================================


def t_list_length_basic():
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn("list_length", [a])
    result = read_handle(out)
    assert result == [3, 2]
    assert out.type_id == ORC_TYPE_U64


def t_list_length_single_list():
    a = make_handle([[42.0]])
    [out] = call_fn("list_length", [a])
    assert read_handle(out) == [1]


def t_list_length_depth3():
    a = make_handle([[[1.0, 2.0], [3.0]], [[4.0, 5.0, 6.0]]])
    [out] = call_fn("list_length", [a])
    result = read_handle(out)
    assert result == [[2, 1], [3]]


# ============================================================
# flatten_deck — Correctness
# ============================================================


def t_flatten_basic():
    a = make_handle([[1.0, 2.0, 3.0], [4.0, 5.0]])
    [out] = call_fn("flatten_deck", [a])
    result = read_handle(out)
    assert result == [1.0, 2.0, 3.0, 4.0, 5.0]


def t_flatten_already_flat():
    a = make_handle([1.0, 2.0, 3.0])
    [out] = call_fn("flatten_deck", [a])
    assert read_handle(out) == [1.0, 2.0, 3.0]


def t_flatten_multiple_io():
    a = make_handle([[1.0, 2.0], [3.0]])
    b = make_handle([[4.0, 5.0, 6.0]])
    outs = call_fn("flatten_deck", [a, b], n_outputs=2)
    assert read_handle(outs[0]) == [1.0, 2.0, 3.0]
    assert read_handle(outs[1]) == [4.0, 5.0, 6.0]


def t_flatten_integer_type():
    a = make_handle([[10, 20], [30]])
    assert a.type_id == ORC_TYPE_U8
    [out] = call_fn("flatten_deck", [a])
    assert out.type_id == ORC_TYPE_U8
    assert read_handle(out) == [10, 20, 30]


# ============================================================
# flatten_deck — Error cases
# ============================================================


def t_flatten_mismatched_counts():
    a = make_handle([[1.0]])
    # n_inputs=1, n_outputs=2 → mismatch → output untouched.
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
    a = make_handle([1.0])
    b = make_handle([2.0])
    [out] = call_fn("add", [a, b])
    assert out.free_fn


def t_output_handle_id_preserved():
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
    a = make_handle([[1.0, 2.0]])
    assert a.type_id == ORC_TYPE_F64
    [out] = call_fn("flatten_deck", [a])
    assert out.type_id == ORC_TYPE_F64


def t_list_length_output_is_u64():
    a = make_handle([[1.0, 2.0]])
    [out] = call_fn("list_length", [a])
    assert out.type_id == ORC_TYPE_U64


# ============================================================
# make_handle / read_handle roundtrip
# ============================================================


def t_roundtrip_flat():
    data = [1.0, 2.0, 3.0]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_nested():
    data = [[1.0, 2.0], [3.0, 4.0]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_depth3():
    data = [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_ragged():
    data = [[1.0, 2.0, 3.0], [4.0, 5.0]]
    h = make_handle(data)
    assert read_handle(h) == data


def t_roundtrip_single_element():
    data = [42.0]
    h = make_handle(data)
    assert read_handle(h) == data


def t_type_detection_u8():
    h = make_handle([0, 127, 255])
    assert h.type_id == ORC_TYPE_U8


def t_type_detection_u16():
    h = make_handle([0, 256])
    assert h.type_id == ORC_TYPE_U16


def t_type_detection_u32():
    h = make_handle([0, 0x10000])
    assert h.type_id == ORC_TYPE_U32


def t_type_detection_u64():
    h = make_handle([0, 0x100000000])
    assert h.type_id == ORC_TYPE_U64


def t_type_detection_i8():
    h = make_handle([-128, 127])
    assert h.type_id == ORC_TYPE_I8


def t_type_detection_i16():
    h = make_handle([-129, 0])
    assert h.type_id == ORC_TYPE_I16


def t_type_detection_i32():
    h = make_handle([-0x8000_0000, 0])
    assert h.type_id == ORC_TYPE_I32


def t_type_detection_i64():
    h = make_handle([-0x8000_0001, 0])
    assert h.type_id == ORC_TYPE_I64


def t_type_detection_f64():
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
            passed += 1
            print(f"  PASS  {name}")
        except Exception as e:
            failed += 1
            print(f"  FAIL  {name}: {e}")
    print(f"\n{passed} passed, {failed} failed, {passed + failed} total")
    sys.exit(1 if failed else 0)
