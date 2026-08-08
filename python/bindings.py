"""Bindings for orc_abi.h."""

import ctypes

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
