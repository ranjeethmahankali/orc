"""Auto-generated ctypes bindings for orc_abi.h. Do not edit."""

import ctypes

ORC_ABI_VERSION = 1
ORC_TYPE_U8 = 0x01
ORC_TYPE_U16 = 0x02
ORC_TYPE_U32 = 0x03
ORC_TYPE_U64 = 0x04
ORC_TYPE_F32 = 0x11
ORC_TYPE_F64 = 0x12
ORC_TYPE_I8 = 0x21
ORC_TYPE_I16 = 0x22
ORC_TYPE_I32 = 0x23
ORC_TYPE_I64 = 0x24
ORC_TYPE_PROXY = 0x30
ORC_MSG_LEVEL_DEBUG = 1
ORC_MSG_LEVEL_INFO = 2
ORC_MSG_LEVEL_WARN = 3
ORC_MSG_LEVEL_ERROR = 4
ORC_MSG_LEVEL_FATAL = 5
ORC_ARGS_VARIADIC = 0xffffffffffffffff
ORC_TYPE_ANY = 0xffffffffffffffff
ORC_DIM_LENGTH = 0
ORC_DIM_MASS = 1
ORC_DIM_TIME = 2
ORC_DIM_ELECTRIC_CURRENT = 3
ORC_DIM_TEMPERATURE = 4
ORC_DIM_SUBSTANCE = 5
ORC_DIM_LUMINOSITY = 6
ORC_NUM_DIMS = 7
ORC_ERROR_NONE = 0
ORC_ERROR_ABI_VERSION_MISMATCH = 0xff01
ORC_ERROR_INVALID_HANDLE = 0xff02
ORC_ERROR_INVALID_DIMENSIONS = 0xff03
ORC_ERROR_TYPE_MISMATCH = 0xff04
ORC_ERROR_INVALID_COMBINATIONS = 0xff05
ORC_ERROR_PLUGIN_ALREADY_INITIALIZED = 0xff06
ORC_ERROR_CONCURRENCY_PROBLEM = 0xff07
ORC_ERROR_INVALID_PROXY = 0xff08
ORC_ERROR_CANNOT_LOAD_PLUGINS = 0xff09
ORC_ERROR_OUT_OF_BOUNDS = 0xff0a
ORC_ERROR_ALLOC_FAILED = 0xff0b
ORC_ERROR_NULL_PTR = 0xff0c
ORC_ERROR_MISSING_CAPABILITY = 0xff0d
ORC_ERROR_UNKNOWN = 0xffff
ORC_DECK_PROXY_COPY_ALL = 0x01
ORC_DECK_PROXY_COPY_ITEMS = 0x02
ORC_DECK_PROXY_SHUFFLE = 0x03

OrcTypeId = ctypes.c_uint64
OrcMessageLevel = ctypes.c_uint8
OrcError = ctypes.c_uint32
OrcProxyType = ctypes.c_uint8


class OrcHandle(ctypes.Structure):
    pass


class OrcTypeInfo(ctypes.Structure):
    pass


class OrcFuncInfo(ctypes.Structure):
    pass


class OrcPlugin(ctypes.Structure):
    pass


class OrcHostMemoryAPI(ctypes.Structure):
    pass


class OrcHostCallbackAPI(ctypes.Structure):
    pass


class OrcHost(ctypes.Structure):
    pass


class OrcMark(ctypes.Structure):
    pass


class OrcItemProxy(ctypes.Structure):
    pass


OrcDims = ctypes.c_int32 * 7

OrcDeckFreeFn = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.POINTER(OrcHandle))

OrcFuncFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64,
                             ctypes.POINTER(OrcHandle), ctypes.c_uint64,
                             ctypes.POINTER(OrcHandle), ctypes.c_uint64)
OrcAllocFn = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_uint64,
                              ctypes.c_uint64)
OrcDeallocFn = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint64,
                                ctypes.c_uint64)
OrcReportProgressFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.c_double)
OrcReportMessageFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.c_uint8,
                                      ctypes.c_char_p)
OrcCheckCancellationFn = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_uint64)
OrcReportIntermediateOutputFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64,
                                                 ctypes.POINTER(OrcHandle))
OrcCreateDeckFromProxyFn = ctypes.CFUNCTYPE(ctypes.c_uint32,
                                            ctypes.POINTER(OrcHandle),
                                            ctypes.c_uint64, ctypes.c_uint8,
                                            ctypes.POINTER(OrcHandle),
                                            ctypes.POINTER(OrcHandle))

OrcHandle._fields_ = [
    ("handle", ctypes.c_uint64),
    ("items", ctypes.c_void_p),
    ("n_items", ctypes.c_uint64),
    ("item_size", ctypes.c_uint64),
    ("marks", ctypes.POINTER(OrcMark)),
    ("stride_offset", ctypes.POINTER(ctypes.c_uint64)),
    ("n_marks", ctypes.c_uint64),
    ("strides", ctypes.POINTER(ctypes.c_uint64)),
    ("type_id", OrcTypeId),
    ("dims", OrcDims),
    ("free_fn", OrcDeckFreeFn),
]

OrcTypeInfo._fields_ = [
    ("type_id", OrcTypeId),
    ("name", ctypes.c_char_p),
    ("desc", ctypes.c_char_p),
]

OrcFuncInfo._fields_ = [
    ("name", ctypes.c_char_p),
    ("desc", ctypes.c_char_p),
    ("n_inputs", ctypes.c_uint64),
    ("n_outputs", ctypes.c_uint64),
    ("input_types", ctypes.POINTER(OrcTypeId)),
    ("output_types", ctypes.POINTER(OrcTypeId)),
    ("func", OrcFuncFn),
]

OrcPlugin._fields_ = [
    ("abi_version", ctypes.c_uint64),
    ("name", ctypes.c_char_p),
    ("desc", ctypes.c_char_p),
    ("types", ctypes.POINTER(OrcTypeInfo)),
    ("n_types", ctypes.c_uint64),
    ("functions", ctypes.POINTER(OrcFuncInfo)),
    ("n_functions", ctypes.c_uint64),
]

OrcHostMemoryAPI._fields_ = [
    ("alloc", OrcAllocFn),
    ("dealloc", OrcDeallocFn),
]

OrcHostCallbackAPI._fields_ = [
    ("report_progress", OrcReportProgressFn),
    ("report_message", OrcReportMessageFn),
    ("check_cancellation", OrcCheckCancellationFn),
    ("report_intermediate_output", OrcReportIntermediateOutputFn),
]

OrcHost._fields_ = [
    ("abi_version", ctypes.c_uint64),
    ("memory_api", OrcHostMemoryAPI),
    ("callbacks", OrcHostCallbackAPI),
    ("create_deck_from_proxy", OrcCreateDeckFromProxyFn),
]

OrcMark._fields_ = [
    ("depth", ctypes.c_uint8),
    ("pos", ctypes.c_uint64),
]

OrcItemProxy._fields_ = [
    ("tree", ctypes.c_uint64),
    ("item", ctypes.c_uint64),
]
