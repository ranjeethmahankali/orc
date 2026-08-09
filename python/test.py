"""
Minimal Python Host.

Minimal Python host that loads a plugin shared library via ctypes
and calls a function (e.g. 'add') on two f64 decks.
"""

import ctypes
import os
import platform
import sys
from bindings import *

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


ORC_CTYPE_MAP = {
    ORC_TYPE_U8: ctypes.c_uint8,
    ORC_TYPE_U16: ctypes.c_uint16,
    ORC_TYPE_U32: ctypes.c_uint32,
    ORC_TYPE_U64: ctypes.c_uint64,
    ORC_TYPE_I8: ctypes.c_int8,
    ORC_TYPE_I16: ctypes.c_int16,
    ORC_TYPE_I32: ctypes.c_int32,
    ORC_TYPE_I64: ctypes.c_int64,
    ORC_TYPE_F32: ctypes.c_float,
    ORC_TYPE_F64: ctypes.c_double,
}


def _detect_type(values):
    """Detect the narrowest ORC type that fits all values."""
    has_float = any(isinstance(v, float) for v in values)
    if has_float:
        return ORC_TYPE_F64
    # All ints. Check sign and range.
    lo = min(values)
    hi = max(values)
    if lo >= 0:
        if hi <= 0xFF:
            return ORC_TYPE_U8
        if hi <= 0xFFFF:
            return ORC_TYPE_U16
        if hi <= 0xFFFFFFFF:
            return ORC_TYPE_U32
        return ORC_TYPE_U64
    else:
        if lo >= -0x80 and hi <= 0x7F:
            return ORC_TYPE_I8
        if lo >= -0x8000 and hi <= 0x7FFF:
            return ORC_TYPE_I16
        if lo >= -0x80000000 and hi <= 0x7FFFFFFF:
            return ORC_TYPE_I32
        return ORC_TYPE_I64


def _intrinsic_depth(data):
    """Count nesting depth along the first element's path."""
    if isinstance(data, list) and data and isinstance(data[0], list):
        return 1 + _intrinsic_depth(data[0])
    return 1


def _push(items, marks, data, depth):
    """Recursively flatten nested lists into items, emitting marks."""
    if isinstance(data, list) and data and isinstance(data[0], list):
        # List of lists: first sublist inherits depth, rest get intrinsic.
        _push(items, marks, data[0], depth)
        for sub in data[1:]:
            _push(items, marks, sub, _intrinsic_depth(sub))
    elif isinstance(data, list):
        # Flat list of leaf values.
        for i, val in enumerate(data):
            d = depth if i == 0 else 0
            if d > 0:
                marks.append(OrcMark(depth=d - 1, pos=len(items)))
            items.append(val)
    else:
        if depth > 0:
            marks.append(OrcMark(depth=depth - 1, pos=len(items)))
        items.append(data)


def _calc_strides(marks):
    """Compute stride_offset and strides arrays from marks."""
    stride_offset = []
    acc = 0
    for m in marks:
        stride_offset.append(acc)
        acc += m.depth
    total = (stride_offset[-1] + marks[-1].depth) if marks else 0
    strides = [0xFFFFFFFFFFFFFFFF] * total
    pegs = []
    for i, m in enumerate(marks):
        d = m.depth
        while len(pegs) < d:
            pegs.append(0)
        for j in range(d):
            peg = pegs[j]
            if peg < i:
                idx = stride_offset[peg] + j
                strides[idx] = min(strides[idx], i - peg)
            pegs[j] = i
    return stride_offset, strides


def make_handle(data):
    """Create an OrcHandle from (possibly nested) lists, like deck![...].

    Detects the element type automatically from the leaf values.
    Supports flat lists, lists of lists, and deeper nesting.
    """
    items = []
    marks = []
    _push(items, marks, data, _intrinsic_depth(data))

    type_id = _detect_type(items)
    ctype = ORC_CTYPE_MAP[type_id]

    arr = (ctype * len(items))(*items)
    h = OrcHandle()
    ctypes.memset(ctypes.addressof(h), 0, ctypes.sizeof(h))
    h.handle = next_handle_id()
    h.items = ctypes.cast(arr, ctypes.c_void_p)
    h.n_items = len(items)
    h.item_size = ctypes.sizeof(ctype)
    h.type_id = type_id
    h._arr = arr  # prevent GC
    if marks:
        marks_arr = (OrcMark * len(marks))(*marks)
        stride_offset, strides = _calc_strides(marks)
        stride_offset_arr = (ctypes.c_uint64 *
                             len(stride_offset))(*stride_offset)
        stride_arr = (ctypes.c_uint64 *
                      len(strides))(*strides) if strides else None
        h.marks = ctypes.cast(marks_arr, ctypes.POINTER(OrcMark))
        h.stride_offset = ctypes.cast(stride_offset_arr,
                                      ctypes.POINTER(ctypes.c_uint64))
        h.n_marks = len(marks)
        if stride_arr:
            h.strides = ctypes.cast(stride_arr,
                                    ctypes.POINTER(ctypes.c_uint64))
        # prevent GC
        h._marks_arr = marks_arr
        h._stride_offset_arr = stride_offset_arr
        h._stride_arr = stride_arr

    return h


def read_handle(h):
    """Read values out of an OrcHandle."""
    ctype = ORC_CTYPE_MAP.get(h.type_id)
    if ctype is None:
        raise ValueError(f"Unknown type_id: {h.type_id:#x}")
    ptr = ctypes.cast(h.items, ctypes.POINTER(ctype))
    return [ptr[i] for i in range(h.n_items)]


REQUIRED_SYMBOLS = [
    "orc_plugin_init",
    "orc_deck_alloc",
    "orc_deck_free",
    "orc_deck_from_proxy",
]


def _shared_lib_ext():
    system = platform.system()
    if system == "Linux":
        return ".so"
    if system == "Darwin":
        return ".dylib"
    if system == "Windows":
        return ".dll"
    raise RuntimeError(f"Unsupported platform: {system}")


def _is_plugin(path):
    """Return True if the shared library exports all required plugin symbols."""
    try:
        lib = ctypes.CDLL(path)
    except OSError:
        return False
    for sym in REQUIRED_SYMBOLS:
        if not hasattr(lib, sym):
            return False
    return True


def load_plugins(search_dir, host):
    """Load all compatible plugin shared libraries from search_dir.

    Returns a list of (lib, OrcPlugin) tuples.
    """
    ext = _shared_lib_ext()
    plugins = []
    if not os.path.isdir(search_dir):
        return plugins
    for f in os.listdir(search_dir):
        if not f.endswith(ext):
            continue
        path = os.path.join(search_dir, f)
        if not _is_plugin(path):
            continue
        lib = ctypes.CDLL(path)
        lib.orc_plugin_init.argtypes = [
            ctypes.POINTER(OrcHost),
            ctypes.POINTER(OrcPlugin),
        ]
        lib.orc_plugin_init.restype = OrcError
        plugin = OrcPlugin()
        err = lib.orc_plugin_init(ctypes.byref(host), ctypes.byref(plugin))
        if err != ORC_ERROR_NONE:
            print(f"Skipping {f}: orc_plugin_init failed ({err:#x})")
            continue
        plugins.append((lib, plugin))
        print(f"Loaded plugin: {f}")
    return plugins


def get_function(plugins, name):
    """Find a function by name across all loaded plugins."""
    for _lib, plugin in plugins:
        for i in range(plugin.n_functions):
            fi = plugin.functions[i]
            if fi.name.decode("utf-8") == name:
                return fi
    raise KeyError(f"Function '{name}' not found in any plugin")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.dirname(script_dir)
    search_dir = os.path.join(project_root, "build", "debug")

    # Build the host
    host = OrcHost()
    host.abi_version = ORC_ABI_VERSION
    host.memory_api.alloc = host_alloc
    host.memory_api.dealloc = host_dealloc
    host.callbacks.report_message = report_message

    # Load all plugins from the search directory
    print(f"Searching for plugins in: {search_dir}")
    plugins = load_plugins(search_dir, host)
    if not plugins:
        print("No plugins found.")
        sys.exit(1)

    print(f"\nLoaded {len(plugins)} plugin(s):")
    for _lib, plugin in plugins:
        print(f"  {plugin.name.decode()}: {plugin.desc.decode()}")
        for i in range(plugin.n_functions):
            fi = plugin.functions[i]
            print(f"    - {fi.name.decode()}: {fi.desc.decode()}")
    print()

    # Call 'add' on two f64 arrays
    add_fn = get_function(plugins, "add")

    a = make_handle([1.0, 2.0, 3.0])
    b = make_handle([10.0, 20.0, 30.0])
    inputs = (OrcHandle * 2)(a, b)

    out = OrcHandle()
    ctypes.memset(ctypes.addressof(out), 0, ctypes.sizeof(out))
    out.handle = next_handle_id()

    add_fn.func(0, inputs, 2, ctypes.byref(out), 1)

    result = read_handle(out)
    print(f"add([1, 2, 3], [10, 20, 30]) = {result}")
    assert result == [11.0, 22.0, 33.0], f"Unexpected: {result}"
    print("PASS")

    # Free output via its free_fn if set
    if out.free_fn:
        out.free_fn(ctypes.byref(out))


if __name__ == "__main__":
    main()
