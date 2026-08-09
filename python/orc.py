"""Helpers for working with orc plugins."""

import ctypes
import os
import platform
from bindings import (OrcDeckFreeFn, OrcHandle, OrcAllocFn, OrcDeallocFn,
                      OrcReportMessageFn, ORC_TYPE_U8, ORC_TYPE_U16,
                      ORC_TYPE_U32, ORC_TYPE_U64, ORC_TYPE_I8, ORC_TYPE_I16,
                      ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_F32, ORC_TYPE_F64,
                      OrcMark, OrcHost, OrcPlugin, OrcError, ORC_ERROR_NONE,
                      ORC_ABI_VERSION)


def _handle_del(self):
    if self.free_fn:
        self.free_fn(ctypes.byref(self))
        self.free_fn = OrcDeckFreeFn(0)  # Set the destructor to null pointer.


OrcHandle.__del__ = _handle_del

# ---------------------------------------------------------------------------
# Host callback implementations
# ---------------------------------------------------------------------------

_alloc_registry = {}


@OrcAllocFn
def host_alloc(size, alignment):
    """Allocate a buffer and keep it alive until host_dealloc is called."""
    buf = (ctypes.c_uint8 * size)()
    ptr = ctypes.addressof(buf)
    _alloc_registry[ptr] = buf
    return ptr


@OrcDeallocFn
def host_dealloc(ptr, size, alignment):
    """Release a previously allocated buffer."""
    _alloc_registry.pop(ptr, None)


@OrcReportMessageFn
def report_message(ctx, level, msg):
    """Print a plugin message to stdout with its severity level."""
    level_names = {1: "DEBUG", 2: "INFO", 3: "WARN", 4: "ERROR", 5: "FATAL"}
    text = msg.decode("utf-8", errors="replace") if msg else ""
    print(f"[{level_names.get(level, 'UNKNOWN')}][{ctx}] {text}")


def default_host():
    """Create an OrcHost with default memory and message callbacks."""
    host = OrcHost()
    host.abi_version = ORC_ABI_VERSION
    host.memory_api.alloc = host_alloc
    host.memory_api.dealloc = host_dealloc
    host.callbacks.report_message = report_message
    return host


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_handle_counter = 0


def next_handle_id():
    """Return a monotonically increasing handle identifier."""
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


def make_handle(data, type_id=None):
    """Create an OrcHandle from (possibly nested) lists, like deck![...].

    Detects the element type automatically from the leaf values.
    Supports flat lists, lists of lists, and deeper nesting.
    Optionally pass type_id to force a specific type.
    """
    items = []
    marks = []
    _push(items, marks, data, _intrinsic_depth(data))

    if type_id is None:
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
    """Read values out of an OrcHandle, reconstructing nested structure."""
    ctype = ORC_CTYPE_MAP.get(h.type_id)
    if ctype is None:
        raise ValueError(f"Unknown type_id: {h.type_id:#x}")
    ptr = ctypes.cast(h.items, ctypes.POINTER(ctype))
    items = [ptr[i] for i in range(h.n_items)]

    if not h.marks or h.n_marks == 0:
        return items

    marks = [(h.marks[i].depth, h.marks[i].pos) for i in range(h.n_marks)]
    max_depth = marks[0][0] + 1

    # Split items into leaf groups at mark positions.
    result = []
    for i, (_, pos) in enumerate(marks):
        end = marks[i + 1][1] if i + 1 < len(marks) else len(items)
        result.append(items[pos:end])

    # Nest bottom-up: at each depth level, group by marks with depth >= d.
    for d in range(1, max_depth):
        boundaries = [0]
        for i in range(1, len(marks)):
            if marks[i][0] >= d:
                boundaries.append(i)
        boundaries.append(len(result))
        result = [
            result[boundaries[b]:boundaries[b + 1]]
            for b in range(len(boundaries) - 1)
        ]

    return result[0] if len(result) == 1 else result


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
    """Return True if the plugin exports all required symbols."""
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
