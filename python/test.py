"""
Minimal Python Host.

Minimal Python host that loads a plugin shared library via ctypes
and calls a function (e.g. 'add') on two f64 decks.
"""

import ctypes
import os
import platform
import sys

# ---------------------------------------------------------------------------
# ABI constants
# ---------------------------------------------------------------------------

ORC_ABI_VERSION = (0 << 42) | (0 << 21) | 1
ORC_TYPE_F64 = 0x12
ORC_ERROR_NONE = 0
ORC_NUM_DIMS = 7

# ---------------------------------------------------------------------------
# ctypes struct definitions (mirrors c/orc_abi.h)
# ---------------------------------------------------------------------------

OrcTypeId = ctypes.c_uint64
OrcError = ctypes.c_uint32
OrcMessageLevel = ctypes.c_uint8
OrcProxyType = ctypes.c_uint8
OrcDims = ctypes.c_int32 * ORC_NUM_DIMS


class OrcHandle(ctypes.Structure):
    pass


OrcDeckFreeFn = ctypes.CFUNCTYPE(OrcError, ctypes.POINTER(OrcHandle))


class OrcMark(ctypes.Structure):
    _fields_ = [
        ("depth", ctypes.c_uint8),
        ("pos", ctypes.c_uint64),
    ]


OrcHandle._fields_ = [
    ("handle", ctypes.c_uint64),
    ("items", ctypes.c_void_p),
    ("n_items", ctypes.c_uint64),
    ("item_size", ctypes.c_uint64),
    ("marks", ctypes.POINTER(OrcMark)),
    ("stride_offset", ctypes.POINTER(ctypes.c_uint64)),
    ("n_marks", ctypes.c_uint64),
    ("strides", ctypes.POINTER(ctypes.c_uint64)),
    ("type_id", ctypes.c_uint64),
    ("dims", OrcDims),
    ("free_fn", OrcDeckFreeFn),
]

# OrcFuncInfo.func signature:
#   void (*func)(uint64_t ctx, OrcHandle const *inputs, uint64_t n_inputs,
#                OrcHandle *outputs, uint64_t n_outputs)
OrcPluginFuncPtr = ctypes.CFUNCTYPE(
    None,
    ctypes.c_uint64,
    ctypes.POINTER(OrcHandle),
    ctypes.c_uint64,
    ctypes.POINTER(OrcHandle),
    ctypes.c_uint64,
)


class OrcTypeInfo(ctypes.Structure):
    _fields_ = [
        ("type_id", ctypes.c_uint64),
        ("name", ctypes.c_char_p),
        ("desc", ctypes.c_char_p),
    ]


class OrcFuncInfo(ctypes.Structure):
    _fields_ = [
        ("name", ctypes.c_char_p),
        ("desc", ctypes.c_char_p),
        ("n_inputs", ctypes.c_uint64),
        ("n_outputs", ctypes.c_uint64),
        ("input_types", ctypes.POINTER(ctypes.c_uint64)),
        ("output_types", ctypes.POINTER(ctypes.c_uint64)),
        ("func", OrcPluginFuncPtr),
    ]


class OrcPlugin(ctypes.Structure):
    _fields_ = [
        ("abi_version", ctypes.c_uint64),
        ("name", ctypes.c_char_p),
        ("desc", ctypes.c_char_p),
        ("types", ctypes.POINTER(OrcTypeInfo)),
        ("n_types", ctypes.c_uint64),
        ("functions", ctypes.POINTER(OrcFuncInfo)),
        ("n_functions", ctypes.c_uint64),
    ]


OrcAllocFn = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_uint64,
                              ctypes.c_uint64)
OrcDeallocFn = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint64,
                                ctypes.c_uint64)


class OrcHostMemoryAPI(ctypes.Structure):
    _fields_ = [
        ("alloc", OrcAllocFn),
        ("dealloc", OrcDeallocFn),
    ]


OrcReportProgressFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.c_double)
OrcReportMessageFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, OrcMessageLevel,
                                      ctypes.c_char_p)
OrcCheckCancellationFn = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_uint64)
OrcReportIntermediateOutputFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64,
                                                 ctypes.POINTER(OrcHandle))


class OrcHostCallbackAPI(ctypes.Structure):
    _fields_ = [
        ("report_progress", OrcReportProgressFn),
        ("report_message", OrcReportMessageFn),
        ("check_cancellation", OrcCheckCancellationFn),
        ("report_intermediate_output", OrcReportIntermediateOutputFn),
    ]


OrcCreateDeckFromProxyFn = ctypes.CFUNCTYPE(
    OrcError,
    ctypes.POINTER(OrcHandle),
    ctypes.c_uint64,
    OrcProxyType,
    ctypes.POINTER(OrcHandle),
    ctypes.POINTER(OrcHandle),
)


class OrcHost(ctypes.Structure):
    _fields_ = [
        ("abi_version", ctypes.c_uint64),
        ("memory_api", OrcHostMemoryAPI),
        ("callbacks", OrcHostCallbackAPI),
        ("create_deck_from_proxy", OrcCreateDeckFromProxyFn),
    ]


# ---------------------------------------------------------------------------
# Host callback implementations
# ---------------------------------------------------------------------------

_alloc_registry = {}


@OrcAllocFn
def host_alloc(size, alignment):
    buf = (ctypes.c_uint8 * size)()
    ptr = ctypes.addressof(buf)
    _alloc_registry[ptr] = buf
    return ptr


@OrcDeallocFn
def host_dealloc(ptr, size, alignment):
    _alloc_registry.pop(ptr, None)


@OrcReportMessageFn
def report_message(ctx, level, msg):
    level_names = {1: "DEBUG", 2: "INFO", 3: "WARN", 4: "ERROR", 5: "FATAL"}
    text = msg.decode("utf-8", errors="replace") if msg else ""
    print(f"[{level_names.get(level, 'UNKNOWN')}][{ctx}] {text}")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_handle_counter = 0


def next_handle_id():
    global _handle_counter
    hid = _handle_counter
    _handle_counter += 1
    return hid


def make_f64_handle(values):
    """Create an OrcHandle backed by a flat f64 array."""
    n = len(values)
    arr = (ctypes.c_double * n)(*values)
    h = OrcHandle()
    ctypes.memset(ctypes.addressof(h), 0, ctypes.sizeof(h))
    h.handle = next_handle_id()
    h.items = ctypes.cast(arr, ctypes.c_void_p)
    h.n_items = n
    h.item_size = ctypes.sizeof(ctypes.c_double)
    h.type_id = ORC_TYPE_F64
    # prevent GC
    h._arr = arr
    return h


def read_f64_handle(h):
    """Read f64 values out of an OrcHandle."""
    ptr = ctypes.cast(h.items, ctypes.POINTER(ctypes.c_double))
    return [ptr[i] for i in range(h.n_items)]


def find_plugin_lib(search_dir):
    """Find the plugin shared library."""
    system = platform.system()
    if system == "Linux":
        prefix, ext = "libdeck_ops", ".so"
    elif system == "Darwin":
        prefix, ext = "libdeck_ops", ".dylib"
    elif system == "Windows":
        prefix, ext = "deck_ops", ".dll"
    else:
        raise RuntimeError(f"Unsupported platform: {system}")
    for f in os.listdir(search_dir):
        if f.startswith(prefix) and f.endswith(ext):
            return os.path.join(search_dir, f)
    return None


def get_function(plugin, name):
    """Find a function by name in the plugin's function table."""
    for i in range(plugin.n_functions):
        fi = plugin.functions[i]
        if fi.name.decode("utf-8") == name:
            return fi
    raise KeyError(f"Function '{name}' not found in plugin")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    search_dirs = [os.path.join(project_root, "build", "debug")]

    lib_path = None
    for d in search_dirs:
        print(f"Searching dir: {d}")
        if os.path.isdir(d):
            lib_path = find_plugin_lib(d)
            if lib_path:
                break

    if not lib_path:
        print(f"Could not find deck_ops plugin library in: {search_dirs}")
        sys.exit(1)

    print(f"Loading plugin: {lib_path}")
    lib = ctypes.CDLL(lib_path)

    lib.orc_plugin_init.argtypes = [
        ctypes.POINTER(OrcHost),
        ctypes.POINTER(OrcPlugin),
    ]
    lib.orc_plugin_init.restype = OrcError

    # Build the host
    host = OrcHost()
    host.abi_version = ORC_ABI_VERSION
    host.memory_api.alloc = host_alloc
    host.memory_api.dealloc = host_dealloc
    host.callbacks.report_message = report_message

    # Init plugin
    plugin = OrcPlugin()
    err = lib.orc_plugin_init(ctypes.byref(host), ctypes.byref(plugin))
    if err != ORC_ERROR_NONE:
        print(f"orc_plugin_init failed: {err:#x}")
        sys.exit(1)

    print(f"Plugin: {plugin.name.decode()}")
    print(f"  {plugin.desc.decode()}")
    print(f"  {plugin.n_functions} function(s):")
    for i in range(plugin.n_functions):
        fi = plugin.functions[i]
        print(f"    - {fi.name.decode()}: {fi.desc.decode()}")
    print()

    # Call 'add' on two f64 arrays
    add_fn = get_function(plugin, "add")

    a = make_f64_handle([1.0, 2.0, 3.0])
    b = make_f64_handle([10.0, 20.0, 30.0])
    inputs = (OrcHandle * 2)(a, b)

    out = OrcHandle()
    ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
    out.handle = next_handle_id()

    add_fn.func(0, inputs, 2, ctypes.byref(out), 1)

    result = read_f64_handle(out)
    print(f"add([1, 2, 3], [10, 20, 30]) = {result}")
    assert result == [11.0, 22.0, 33.0], f"Unexpected: {result}"
    print("PASS")

    # Free output via its free_fn if set
    if out.free_fn:
        out.free_fn(ctypes.byref(out))


if __name__ == "__main__":
    main()
