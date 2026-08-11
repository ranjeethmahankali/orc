"""Helpers for working with orc plugins."""

import ctypes
import os
import platform
import sys
import numpy as np
from bindings import (OrcDeckFreeFn, OrcHandle, OrcAllocFn, OrcDeallocFn,
                      OrcReportMessageFn, ORC_TYPE_U8, ORC_TYPE_U16,
                      ORC_TYPE_U32, ORC_TYPE_U64, ORC_TYPE_I8, ORC_TYPE_I16,
                      ORC_TYPE_I32, ORC_TYPE_I64, ORC_TYPE_F32, ORC_TYPE_F64,
                      OrcMark, OrcHost, OrcPlugin, OrcError, ORC_ERROR_NONE,
                      ORC_ABI_VERSION, OrcCreateDeckFromProxyFn, OrcItemProxy,
                      ORC_ERROR_INVALID_HANDLE, ORC_ERROR_INVALID_PROXY,
                      ORC_DECK_PROXY_COPY_ALL, ORC_DECK_PROXY_COPY_ITEMS,
                      ORC_DECK_PROXY_SHUFFLE, ORC_ARGS_VARIADIC)


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


_proxy_deck_registry = {}  # handle_id -> tuple of backing ctypes objects
_loaded_plugins = []  # populated by load_plugins


@OrcDeckFreeFn
def _proxy_deck_free(handle_ptr):
    if not handle_ptr:
        return ORC_ERROR_NONE
    _proxy_deck_registry.pop(handle_ptr[0].handle, None)
    return ORC_ERROR_NONE


def _read_handle_items(h, ctype):
    ptr = ctypes.cast(h.items, ctypes.POINTER(ctype))
    return [ptr[i] for i in range(h.n_items)]


def _read_handle_marks(h):
    if not h.marks or h.n_marks == 0:
        return []
    return [OrcMark(depth=h.marks[i].depth, pos=h.marks[i].pos)
            for i in range(h.n_marks)]


@OrcCreateDeckFromProxyFn
def host_create_proxy_deck(inputs_ptr, n_inputs, proxy_type, proxy_ptr, out_ptr):
    if not inputs_ptr or not proxy_ptr or not out_ptr:
        return ORC_ERROR_INVALID_HANDLE
    inputs = [inputs_ptr[i] for i in range(n_inputs)]
    if not inputs:
        return ORC_ERROR_INVALID_PROXY
    type_id = inputs[0].type_id
    if any(h.type_id != type_id for h in inputs[1:]):
        return ORC_ERROR_INVALID_PROXY

    proxy = proxy_ptr[0]
    ctype = ORC_CTYPE_MAP.get(type_id)

    if ctype is None:
        # Plugin type — find the owning plugin and dispatch to it.
        for lib, plugin in _loaded_plugins:
            for j in range(plugin.n_types):
                if plugin.types[j].type_id == type_id:
                    lib.orc_deck_from_proxy.argtypes = [
                        ctypes.POINTER(OrcHandle), ctypes.c_uint64,
                        ctypes.c_uint8,
                        ctypes.POINTER(OrcHandle), ctypes.POINTER(OrcHandle),
                    ]
                    lib.orc_deck_from_proxy.restype = OrcError
                    return lib.orc_deck_from_proxy(
                        inputs_ptr, n_inputs, proxy_type, proxy_ptr, out_ptr)
        return ORC_ERROR_INVALID_PROXY

    # Primitive type — implement proxy operations in Python.
    proxy_marks = _read_handle_marks(proxy)

    if proxy_type == ORC_DECK_PROXY_COPY_ALL:
        if n_inputs != 1:
            return ORC_ERROR_INVALID_PROXY
        items = _read_handle_items(inputs[0], ctype)
        marks = _read_handle_marks(inputs[0])
    elif proxy_type == ORC_DECK_PROXY_COPY_ITEMS:
        if n_inputs != 1:
            return ORC_ERROR_INVALID_PROXY
        items = _read_handle_items(inputs[0], ctype)
        marks = proxy_marks
    elif proxy_type == ORC_DECK_PROXY_SHUFFLE:
        input_items = [_read_handle_items(h, ctype) for h in inputs]
        proxy_item_ptr = ctypes.cast(proxy.items, ctypes.POINTER(OrcItemProxy))
        items = [input_items[proxy_item_ptr[k].tree][proxy_item_ptr[k].item]
                 for k in range(proxy.n_items)]
        marks = proxy_marks
    else:
        return ORC_ERROR_INVALID_PROXY

    arr = (ctype * len(items))(*items)
    handle_id = out_ptr[0].handle
    backing = [arr]

    out_ptr[0].items = ctypes.cast(arr, ctypes.c_void_p)
    out_ptr[0].n_items = len(items)
    out_ptr[0].item_size = ctypes.sizeof(ctype)
    out_ptr[0].type_id = type_id
    out_ptr[0].dims = proxy.dims

    if marks:
        marks_arr = (OrcMark * len(marks))(*marks)
        stride_offset, strides = _calc_strides(list(marks_arr))
        stride_offset_arr = (ctypes.c_uint64 * len(stride_offset))(*stride_offset)
        stride_arr = (ctypes.c_uint64 * len(strides))(*strides) if strides else None
        out_ptr[0].marks = ctypes.cast(marks_arr, ctypes.POINTER(OrcMark))
        out_ptr[0].stride_offset = ctypes.cast(stride_offset_arr,
                                               ctypes.POINTER(ctypes.c_uint64))
        out_ptr[0].n_marks = len(marks)
        if stride_arr:
            out_ptr[0].strides = ctypes.cast(stride_arr,
                                             ctypes.POINTER(ctypes.c_uint64))
        backing += [marks_arr, stride_offset_arr, stride_arr]

    out_ptr[0].free_fn = _proxy_deck_free
    _proxy_deck_registry[handle_id] = tuple(b for b in backing if b is not None)
    return ORC_ERROR_NONE


def default_host():
    """Create an OrcHost with default memory and message callbacks."""
    host = OrcHost()
    host.abi_version = ORC_ABI_VERSION
    host.memory_api.alloc = host_alloc
    host.memory_api.dealloc = host_dealloc
    host.callbacks.report_message = report_message
    host.create_deck_from_proxy = host_create_proxy_deck
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


def empty_handle():
    """Create a zeroed OrcHandle with a fresh handle ID."""
    h = OrcHandle()
    ctypes.memset(ctypes.addressof(h), 0, ctypes.sizeof(h))
    h.handle = next_handle_id()
    return h


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

ORC_NUMPY_DTYPE_MAP = {
    ORC_TYPE_U8: np.uint8,
    ORC_TYPE_U16: np.uint16,
    ORC_TYPE_U32: np.uint32,
    ORC_TYPE_U64: np.uint64,
    ORC_TYPE_I8: np.int8,
    ORC_TYPE_I16: np.int16,
    ORC_TYPE_I32: np.int32,
    ORC_TYPE_I64: np.int64,
    ORC_TYPE_F32: np.float32,
    ORC_TYPE_F64: np.float64,
}


def as_numpy(h):
    """Return a numpy array viewing the handle's items buffer. Zero copy.

    The returned array holds a reference to the handle, preventing
    use-after-free if the handle goes out of scope.
    """
    dtype = ORC_NUMPY_DTYPE_MAP.get(h.type_id)
    if dtype is None:
        raise ValueError(f"Unknown type_id: {h.type_id:#x}")
    ctype = ORC_CTYPE_MAP[h.type_id]
    ptr = ctypes.cast(h.items, ctypes.POINTER(ctype * h.n_items))
    buf = ptr.contents
    # Stash the handle reference on the ctypes buffer. The numpy array
    # holds buf as its base, so the handle stays alive as long as the
    # array does.
    buf._orc_handle = h
    return np.frombuffer(buf, dtype=dtype)


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


def make_deck(data, type_id=None):
    """Create an OrcHandle from (possibly nested) lists, like deck![...].

    Detects the element type automatically from the leaf values.
    Supports flat lists, lists of lists, and deeper nesting.
    Optionally pass type_id to force a specific type.
    """
    items = []
    marks = []
    if not isinstance(data, list):
        items.append(data)
    else:
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


def read_deck(h):
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


def load_plugins(search_dir, verbose=False):
    """Load all compatible plugin shared libraries from search_dir.

    Returns a list of (lib, OrcPlugin) tuples.
    """
    host = default_host()
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
        if verbose:
            print(f"Loaded plugin: {f}")
    global _loaded_plugins
    _loaded_plugins.extend(plugins)
    if verbose:
        print(f"\nLoaded {len(plugins)} plugin(s):")
        for _lib, plugin in plugins:
            print(f"  {plugin.name.decode()}: {plugin.desc.decode()}")
            for i in range(plugin.n_functions):
                fi = plugin.functions[i]
                print(f"    - {fi.name.decode()}: {fi.desc.decode()}")
        print()
    module = sys.modules[__name__]
    for _lib, plugin in plugins:
        for i in range(plugin.n_functions):
            fi = plugin.functions[i]
            wrapper = OrcFuncWrapper(fi, fi.n_inputs, fi.n_outputs)
            setattr(module, wrapper.name, wrapper)


class OrcFuncWrapper:
    """Callable wrapper around a plugin function."""

    def __init__(self, fi, n_inputs, n_outputs):
        """Wrap an OrcFuncInfo as a callable."""
        self._fi = fi
        self.name = fi.name.decode("utf-8")
        self.n_inputs = None if n_inputs == ORC_ARGS_VARIADIC else n_inputs
        self.n_outputs = None if n_outputs == ORC_ARGS_VARIADIC else n_outputs

    def __call__(self, *inputs, n_out=None):
        """Call the plugin function with the given input handles."""
        if self.n_inputs is not None and len(inputs) != self.n_inputs:
            raise ValueError(f"The function '{self.name}' expects {self.n_inputs} arguments.")
        if self.n_outputs is not None and n_out is not None and self.n_outputs != n_out:
            raise ValueError(f"The function '{self.name}' will produce {self.n_outputs} outputs.")
        in_arr = (OrcHandle * len(inputs))(*inputs)
        if n_out is None:
            if self.n_outputs is None:
                # The user nor the function tell us how many outputs the function has.
                # So we assume 1 as the default.
                n_out = 1
            else:
                n_out = self.n_outputs
        outs = [empty_handle() for _ in range(n_out)]
        out_arr = (OrcHandle * n_out)(*outs)
        self._fi.func(0, in_arr, len(inputs), out_arr, n_out)
        if n_out == 1:
            return out_arr[0]
        return [out_arr[i] for i in range(n_out)]

    def __repr__(self):
        """Return a string representation of the function."""
        return f"OrcFunc({self.name!r})"


def get_function(plugins, name):
    """Find a plugin function by name and return a callable OrcFunc."""
    for _lib, plugin in plugins:
        for i in range(plugin.n_functions):
            fi = plugin.functions[i]
            if fi.name.decode("utf-8") == name:
                return OrcFuncWrapper(fi, fi.n_inputs, fi.n_outputs)
    raise KeyError(f"Function '{name}' not found in any plugin")
