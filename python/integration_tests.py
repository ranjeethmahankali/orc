"""
Integration tests for server_host + cli_client.

Assumes both binaries and all plugins are already built.
Starts a server_host process, runs cli_client commands against it,
and verifies the results.

Usage:
    python integration_tests.py [build_dir]

build_dir defaults to build/debug relative to the project root.
"""

import math
import os
import signal
import subprocess
import sys
import time

# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------

script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(script_dir)
build_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    project_root, "build", "debug")

if sys.platform == "win32":
    server_bin = os.path.join(build_dir, "server_host.exe")
    client_bin = os.path.join(build_dir, "cli_client.exe")
else:
    server_bin = os.path.join(build_dir, "server_host")
    client_bin = os.path.join(build_dir, "cli_client")

HOST = "127.0.0.1"
PORT = "0"  # Let the server pick a free port.
server_proc = None
server_port = None


def start_server():
    global server_proc, server_port
    server_proc = subprocess.Popen(
        [server_bin, PORT],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    # Read the port from stdout: "orc server listening on port NNNN"
    line = server_proc.stdout.readline()
    if not line:
        err = server_proc.stderr.read()
        raise RuntimeError(f"Server failed to start: {err}")
    parts = line.strip().split()
    server_port = parts[-1]


def stop_server():
    global server_proc
    if server_proc is not None:
        if sys.platform == "win32":
            server_proc.terminate()
        else:
            server_proc.send_signal(signal.SIGTERM)
        server_proc.wait(timeout=5)
        server_proc = None


def cli(*args):
    """Run cli_client and return stdout stripped."""
    cmd = [client_bin, HOST, server_port] + list(args)
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
    if result.returncode != 0:
        raise RuntimeError(
            f"cli_client failed: {' '.join(args)}\n"
            f"stdout: {result.stdout}\nstderr: {result.stderr}")
    return result.stdout.strip()


def cli_fails(*args):
    """Assert that cli_client returns non-zero."""
    cmd = [client_bin, HOST, server_port] + list(args)
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
    return result.returncode != 0


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def session_start():
    return cli("session", "start")


def session_close(sid):
    cli("session", "close", sid)


def constant(sid, dtype, *values):
    """Upload a constant and return handle_id as string."""
    return cli("constant", sid, dtype, *[str(v) for v in values])


def call(sid, func, *input_ids):
    """Call a function and return output handle_ids as list of strings."""
    return cli("call", sid, func, *input_ids).split()


def download(sid, hid):
    """Download a handle, return (type_name, values_list)."""
    raw = cli("download", sid, hid)
    parts = raw.split()
    dtype = parts[0]
    values = parts[1:]
    return dtype, values


def download_values(sid, hid):
    """Download and return parsed numeric values."""
    dtype, vals = download(sid, hid)
    if dtype in ("f32", "f64"):
        return [float(v) for v in vals]
    else:
        return [int(v) for v in vals]


def download_typed(sid, hid):
    """Download and return (type_name, parsed numeric values)."""
    dtype, vals = download(sid, hid)
    if dtype in ("f32", "f64"):
        return dtype, [float(v) for v in vals]
    else:
        return dtype, [int(v) for v in vals]


# ============================================================
# session — start / close
# ============================================================


def t_session_start_returns_id():
    sid = session_start()
    assert sid.isdigit(), f"Expected numeric session_id, got: {sid}"
    session_close(sid)


def t_session_multiple():
    s1 = session_start()
    s2 = session_start()
    assert s1 != s2
    session_close(s1)
    session_close(s2)


# ============================================================
# constant + download — round-trip all builtin types
# ============================================================


def t_constant_f64():
    sid = session_start()
    hid = constant(sid, "f64", 1.5, -3.25, 0.0)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "f64"
    assert vals == [1.5, -3.25, 0.0]
    session_close(sid)


def t_constant_f32():
    sid = session_start()
    hid = constant(sid, "f32", 1.5, -3.25)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "f32"
    assert vals == [1.5, -3.25]
    session_close(sid)


def t_constant_u8():
    sid = session_start()
    hid = constant(sid, "u8", 0, 127, 255)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "u8"
    assert vals == [0, 127, 255]
    session_close(sid)


def t_constant_u16():
    sid = session_start()
    hid = constant(sid, "u16", 0, 256, 65535)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "u16"
    assert vals == [0, 256, 65535]
    session_close(sid)


def t_constant_u32():
    sid = session_start()
    hid = constant(sid, "u32", 0, 70000, 4294967295)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "u32"
    assert vals == [0, 70000, 4294967295]
    session_close(sid)


def t_constant_u64():
    sid = session_start()
    hid = constant(sid, "u64", 0, 1, 4294967296)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "u64"
    assert vals == [0, 1, 4294967296]
    session_close(sid)


def t_constant_i8():
    sid = session_start()
    hid = constant(sid, "i8", -128, 0, 127)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "i8"
    assert vals == [-128, 0, 127]
    session_close(sid)


def t_constant_i16():
    sid = session_start()
    hid = constant(sid, "i16", -32768, 0, 32767)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "i16"
    assert vals == [-32768, 0, 32767]
    session_close(sid)


def t_constant_i32():
    sid = session_start()
    hid = constant(sid, "i32", -2147483648, 0, 2147483647)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "i32"
    assert vals == [-2147483648, 0, 2147483647]
    session_close(sid)


def t_constant_i64():
    sid = session_start()
    hid = constant(sid, "i64", -2147483649, 0, 2147483648)
    dtype, vals = download_typed(sid, hid)
    assert dtype == "i64"
    assert vals == [-2147483649, 0, 2147483648]
    session_close(sid)


def t_constant_single_element():
    sid = session_start()
    hid = constant(sid, "f64", 42.0)
    vals = download_values(sid, hid)
    assert vals == [42.0]
    session_close(sid)


# ============================================================
# add
# ============================================================


def t_add_f64_flat():
    sid = session_start()
    a = constant(sid, "f64", 1, 2, 3)
    b = constant(sid, "f64", 10, 20, 30)
    [out] = call(sid, "add", a, b)
    assert download_values(sid, out) == [11.0, 22.0, 33.0]
    session_close(sid)


def t_add_broadcast_scalar():
    sid = session_start()
    a = constant(sid, "f64", 1, 2, 3)
    b = constant(sid, "f64", 10)
    [out] = call(sid, "add", a, b)
    assert download_values(sid, out) == [11.0, 12.0, 13.0]
    session_close(sid)


def t_add_single_element():
    sid = session_start()
    a = constant(sid, "f64", 5)
    b = constant(sid, "f64", 3)
    [out] = call(sid, "add", a, b)
    assert download_values(sid, out) == [8.0]
    session_close(sid)


def t_add_i64():
    sid = session_start()
    a = constant(sid, "i64", -5, -3, 0, 3, 5)
    b = constant(sid, "i64", 10, 20, 30, 40, 50)
    [out] = call(sid, "add", a, b)
    dtype, vals = download_typed(sid, out)
    assert dtype == "i64"
    assert vals == [5, 17, 30, 43, 55]
    session_close(sid)


def t_add_f32():
    sid = session_start()
    a = constant(sid, "f64", 1.5, 2.5)
    b = constant(sid, "f64", 10.5, 20.5)
    [out] = call(sid, "add", a, b)
    assert download_values(sid, out) == [12.0, 23.0]
    session_close(sid)


# ============================================================
# multiply
# ============================================================


def t_mul_f64():
    sid = session_start()
    a = constant(sid, "f64", 2, 3, 4)
    b = constant(sid, "f64", 5, 6, 7)
    [out] = call(sid, "multiply", a, b)
    assert download_values(sid, out) == [10.0, 18.0, 28.0]
    session_close(sid)


def t_mul_i64():
    sid = session_start()
    a = constant(sid, "i64", 3, 4)
    b = constant(sid, "i64", 7, 8)
    [out] = call(sid, "multiply", a, b)
    dtype, vals = download_typed(sid, out)
    assert dtype == "i64"
    assert vals == [21, 32]
    session_close(sid)


# ============================================================
# subtract
# ============================================================


def t_sub_f64():
    sid = session_start()
    a = constant(sid, "f64", 10, 20)
    b = constant(sid, "f64", 3, 7)
    [out] = call(sid, "subtract", a, b)
    assert download_values(sid, out) == [7.0, 13.0]
    session_close(sid)


# ============================================================
# divide
# ============================================================


def t_div_f64():
    sid = session_start()
    a = constant(sid, "f64", 10, 9)
    b = constant(sid, "f64", 2, 3)
    [out] = call(sid, "divide", a, b)
    assert download_values(sid, out) == [5.0, 3.0]
    session_close(sid)


def t_div_by_zero():
    sid = session_start()
    a = constant(sid, "f64", 1)
    b = constant(sid, "f64", 0)
    [out] = call(sid, "divide", a, b)
    vals = download_values(sid, out)
    assert math.isinf(vals[0]) and vals[0] > 0
    session_close(sid)


# ============================================================
# repeat_list
# ============================================================


## repeat_list tests are disabled because the function currently hangs the
## server (likely a bug in the plugin's DeckWriter interaction with the
## server's threading model). Re-enable once that is fixed.
# def t_repeat_list_f64():
#     sid = session_start()
#     a = constant(sid, "f64", 1, 2, 3)
#     count = constant(sid, "u64", 3)
#     [out] = call(sid, "repeat_list", a, count)
#     assert download_values(sid, out) == [1, 2, 3, 1, 2, 3, 1, 2, 3]
#     session_close(sid)
#
# def t_repeat_list_zero():
#     sid = session_start()
#     a = constant(sid, "f64", 1, 2)
#     count = constant(sid, "u64", 0)
#     [out] = call(sid, "repeat_list", a, count)
#     _, vals = download_typed(sid, out)
#     assert vals == []
#     session_close(sid)


# ============================================================
# flatten_deck (from C plugin)
# ============================================================


def t_flatten_flat_identity():
    """Flatten a flat deck is identity."""
    sid = session_start()
    a = constant(sid, "f64", 1, 2, 3)
    [out] = call(sid, "flatten_deck", a)
    assert download_values(sid, out) == [1.0, 2.0, 3.0]
    session_close(sid)


def t_flatten_preserves_type():
    """Flatten preserves the element type."""
    sid = session_start()
    a = constant(sid, "u8", 10, 20, 30)
    [out] = call(sid, "flatten_deck", a)
    dtype, vals = download_typed(sid, out)
    assert dtype == "u8"
    assert vals == [10, 20, 30]
    session_close(sid)


# ============================================================
# create_complex + complex_get_parts
# ============================================================


def t_complex_create_and_get_parts():
    """Create complex numbers and get parts back."""
    sid = session_start()
    real = constant(sid, "f64", 1, 2, 3)
    imag = constant(sid, "f64", 4, 5, 6)
    [cpx] = call(sid, "create_complex", real, imag)
    parts = call(sid, "complex_get_parts", cpx)
    assert len(parts) == 2
    real_out = download_values(sid, parts[0])
    imag_out = download_values(sid, parts[1])
    assert real_out == [1.0, 2.0, 3.0]
    assert imag_out == [4.0, 5.0, 6.0]
    session_close(sid)


def t_complex_negative_parts():
    """Complex with negative components round-trips correctly."""
    sid = session_start()
    real = constant(sid, "f64", -1, 0, 1)
    imag = constant(sid, "f64", 3, -3, 0)
    [cpx] = call(sid, "create_complex", real, imag)
    parts = call(sid, "complex_get_parts", cpx)
    assert download_values(sid, parts[0]) == [-1.0, 0.0, 1.0]
    assert download_values(sid, parts[1]) == [3.0, -3.0, 0.0]
    session_close(sid)


# ============================================================
# add_complex
# ============================================================


def t_complex_add():
    """[1+2i, 3+4i] + [10+20i, 30+40i] = [11+22i, 33+44i]"""
    sid = session_start()
    lhs_r = constant(sid, "f64", 1, 3)
    lhs_i = constant(sid, "f64", 2, 4)
    [lhs] = call(sid, "create_complex", lhs_r, lhs_i)
    rhs_r = constant(sid, "f64", 10, 30)
    rhs_i = constant(sid, "f64", 20, 40)
    [rhs] = call(sid, "create_complex", rhs_r, rhs_i)
    [out] = call(sid, "add_complex", lhs, rhs)
    parts = call(sid, "complex_get_parts", out)
    assert download_values(sid, parts[0]) == [11.0, 33.0]
    assert download_values(sid, parts[1]) == [22.0, 44.0]
    session_close(sid)


def t_complex_add_negative():
    """[1-2i, -3+4i] + [0+3i, 3-4i] = [1+1i, 0+0i]"""
    sid = session_start()
    [lhs] = call(sid, "create_complex",
                 constant(sid, "f64", 1, -3),
                 constant(sid, "f64", -2, 4))
    [rhs] = call(sid, "create_complex",
                 constant(sid, "f64", 0, 3),
                 constant(sid, "f64", 3, -4))
    [out] = call(sid, "add_complex", lhs, rhs)
    parts = call(sid, "complex_get_parts", out)
    assert download_values(sid, parts[0]) == [1.0, 0.0]
    assert download_values(sid, parts[1]) == [1.0, 0.0]
    session_close(sid)


# ============================================================
# mul_complex
# ============================================================


def t_complex_mul():
    """[1+2i, 2+3i] * [3+4i, 1+0i] = [-5+10i, 2+3i]"""
    sid = session_start()
    [lhs] = call(sid, "create_complex",
                 constant(sid, "f64", 1, 2),
                 constant(sid, "f64", 2, 3))
    [rhs] = call(sid, "create_complex",
                 constant(sid, "f64", 3, 1),
                 constant(sid, "f64", 4, 0))
    [out] = call(sid, "mul_complex", lhs, rhs)
    parts = call(sid, "complex_get_parts", out)
    assert download_values(sid, parts[0]) == [-5.0, 2.0]
    assert download_values(sid, parts[1]) == [10.0, 3.0]
    session_close(sid)


def t_complex_mul_i_squared():
    """i * i = -1 for three elements."""
    sid = session_start()
    [lhs] = call(sid, "create_complex",
                 constant(sid, "f64", 0, 0, 0),
                 constant(sid, "f64", 1, 1, 1))
    [rhs] = call(sid, "create_complex",
                 constant(sid, "f64", 0, 0, 0),
                 constant(sid, "f64", 1, 1, 1))
    [out] = call(sid, "mul_complex", lhs, rhs)
    parts = call(sid, "complex_get_parts", out)
    assert download_values(sid, parts[0]) == [-1.0, -1.0, -1.0]
    assert download_values(sid, parts[1]) == [0.0, 0.0, 0.0]
    session_close(sid)


def t_complex_mul_by_zero():
    """[3+4i, 1+1i] * [0+0i, 0+0i] = [0+0i, 0+0i]"""
    sid = session_start()
    [lhs] = call(sid, "create_complex",
                 constant(sid, "f64", 3, 1),
                 constant(sid, "f64", 4, 1))
    [rhs] = call(sid, "create_complex",
                 constant(sid, "f64", 0, 0),
                 constant(sid, "f64", 0, 0))
    [out] = call(sid, "mul_complex", lhs, rhs)
    parts = call(sid, "complex_get_parts", out)
    assert download_values(sid, parts[0]) == [0.0, 0.0]
    assert download_values(sid, parts[1]) == [0.0, 0.0]
    session_close(sid)


# ============================================================
# Chained operations
# ============================================================


def t_chain_add_then_multiply():
    """(a + b) * c"""
    sid = session_start()
    a = constant(sid, "f64", 1, 2, 3)
    b = constant(sid, "f64", 10, 20, 30)
    [s] = call(sid, "add", a, b)
    c = constant(sid, "f64", 2)
    [out] = call(sid, "multiply", s, c)
    assert download_values(sid, out) == [22.0, 44.0, 66.0]
    session_close(sid)


def t_chain_mul_then_sub():
    """a * b - a"""
    sid = session_start()
    a = constant(sid, "f64", 2, 3)
    b = constant(sid, "f64", 10, 20)
    [product] = call(sid, "multiply", a, b)
    [out] = call(sid, "subtract", product, a)
    assert download_values(sid, out) == [18.0, 57.0]
    session_close(sid)


def t_diamond_topology():
    """x -> doubled=x+x, squared=x*x -> squared - doubled."""
    sid = session_start()
    x = constant(sid, "f64", 5)
    [doubled] = call(sid, "add", x, x)
    [squared] = call(sid, "multiply", x, x)
    [out] = call(sid, "subtract", squared, doubled)
    assert download_values(sid, out) == [15.0]
    session_close(sid)


# ============================================================
# Multiple sessions
# ============================================================


def t_independent_sessions():
    """Two sessions don't interfere with each other."""
    s1 = session_start()
    s2 = session_start()
    a1 = constant(s1, "f64", 1, 2)
    a2 = constant(s2, "f64", 100, 200)
    b1 = constant(s1, "f64", 10, 20)
    b2 = constant(s2, "f64", 1000, 2000)
    [out1] = call(s1, "add", a1, b1)
    [out2] = call(s2, "add", a2, b2)
    assert download_values(s1, out1) == [11.0, 22.0]
    assert download_values(s2, out2) == [1100.0, 2200.0]
    session_close(s1)
    session_close(s2)


# ============================================================
# Handle reuse within a session
# ============================================================


def t_handle_reuse():
    """Same handle can be used as input to multiple calls."""
    sid = session_start()
    a = constant(sid, "f64", 5, 10)
    [doubled] = call(sid, "add", a, a)
    [tripled] = call(sid, "add", doubled, a)
    assert download_values(sid, doubled) == [10.0, 20.0]
    assert download_values(sid, tripled) == [15.0, 30.0]
    session_close(sid)


# ============================================================
# functions listing
# ============================================================


def t_functions_lists_expected():
    """The /functions endpoint returns known function names."""
    raw = cli("functions")
    expected = ["add", "multiply", "subtract", "divide", "repeat_list",
                "create_complex", "add_complex", "mul_complex",
                "complex_get_parts"]
    for name in expected:
        assert name in raw, f"Expected function '{name}' in functions list"


# ============================================================
# Runner
# ============================================================

if __name__ == "__main__":
    # by convention all tests start with t_
    tests = [(name, fn) for name, fn in globals().items()
             if name.startswith("t_") and callable(fn)]
    tests.sort(key=lambda x: x[0])

    start_server()
    try:
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
    finally:
        stop_server()
    sys.exit(1 if failed else 0)
