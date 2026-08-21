"""Auto-generated ctypes bindings for orc_abi.h. Do not edit."""

import ctypes

ORC_ABI_VERSION = 1

# Unsigned integers.
ORC_TYPE_U8 = 0x01
ORC_TYPE_U16 = 0x02
ORC_TYPE_U32 = 0x03
ORC_TYPE_U64 = 0x04
# Scalars.
ORC_TYPE_F32 = 0x11
ORC_TYPE_F64 = 0x12
# Signed integers.
ORC_TYPE_I8 = 0x21
ORC_TYPE_I16 = 0x22
ORC_TYPE_I32 = 0x23
ORC_TYPE_I64 = 0x24
# Proxy for an item in a tree.
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
ORC_ERROR_INVALID_FUNCTION = 0xff0e
ORC_ERROR_INVALID_ARGUMENTS = 0xff0f
ORC_ERROR_SERIALIZATION_ERROR = 0xff10
ORC_ERROR_INVALID_CONTEXT = 0xff11
ORC_ERROR_UNKNOWN = 0xffff

ORC_DECK_PROXY_COPY_ALL = 0x01
ORC_DECK_PROXY_COPY_ITEMS = 0x02
ORC_DECK_PROXY_SHUFFLE = 0x03


OrcTypeId = ctypes.c_uint64
OrcMessageLevel = ctypes.c_uint8
OrcError = ctypes.c_uint32
OrcProxyType = ctypes.c_uint8

class OrcHandle(ctypes.Structure):
    """This is the primary handle passed around between hosts and plugins as input and outputs for the plugin function calls. - handle: This is a unique integer assigned by the host program. The plugin MUST NOT modify it. The plugin may use it as a key to point to data allocated within it's memory, when said data is owned by the same plugin. The host uses this integer to keep track of all the data alive during a session. - items, and n_items: These are the actual data stored in the deck that this handles points to. This handle can be backed by any implementation of a Deck datastructure, written in any language, as long as these pointers meet the ABI requirements to be read across the FFI boundary. - marks, stride_offset, and n_mmarks: These define the structure / nesting inside a deck. - type_id: Indicates the type of the data stored in the deck backs this handle. - dims: Optional units. Just metadata, the plugin functions and host can do whatever they want with it. - free_fn: Destructor. Whichever plugin allocates the backing deck, and populates this handle with the corresponding pointers, is also responsible for setting the destructor function pointer. Without this, the memory may leak."""

class OrcTypeInfo(ctypes.Structure):
    """This is just a tag used to identify the types of inputs and outputs of plugin functions. `OrcTypeId`, is the actual tag used by the host and plugin to distinguish the types. `name` and `desc` are just optional information to help the host / user understand what the type is."""

class OrcFuncInfo(ctypes.Structure):
    """Metadata for a function exposed by the plugin. All plugin functions have the same signature. The metadata encodes information about the inputs, outputs, and the function pointer. `name` and `desc` are optional strings that are meant to help the host / user understand what the function does. `n_inputs` and `n_outputs` can be any value, except `ORC_ARGS_VARIADIC`. If they are set to `ORC_ARGS_VARIADIC`, host will infer that the functions will support any number of inputs / outputs. If `n_inputs` / `n_outputs` are set to a value other than `ORC_ARGS_VARIADIC`, then the host can read the corresponding `input_types` and `output_types` arrays to infer the types expected by the function. This should only be set if the function expects one concrete type. If the function is generic / and can process many types, the corresponding type must be set to `ORC_TYPE_ANY`. If the `input_types` and `output_types` pointers are set to `NULL`, the host must infer that all inputs and outputs can be of any type. This metadata is a hint for the host. The plugin function must still validate all it's inputs, and outputs, counts, and types, and return appropriate errors. Conversely, even if the metadata implies a certain input/output configuration is supported by the function, the function may still fail when called with said inputs/outputs."""

class OrcPlugin(ctypes.Structure):
    """This struct is how the plugin communicates to the host about itself. - abi_version: Must be populated with the that of the header that was used to compile the plugin. - name and desc: Optional information to help the host / user understand what the plugin is about. - types and n_types: Array of custom types that used by the functions of this plugin. It can be `NULL` if `n_types` is zero. The plugin must not redeclare any of the primitive types, nor can its types conflict with the types of other types loaded by other plugins. The plugin is advised to choose a randmly generated 64 bit string as it's `OrcTypeId`, so that it doesn't conflict with any other plugin's types. - functions and n_functions: Array of functions exposed by this plugin. The pointer can be `NULL` if n_types is zero."""

class OrcHostMemoryAPI(ctypes.Structure):
    """Optionally the host may provide a memory allocator for the plugin. This struct is how the host passes the function pointers to the plugin. The host may choose to leave these `NULL`, and leave the plugin to use it's own allocator."""

class OrcHostCallbackAPI(ctypes.Structure):
    """The host may populate any of these function pointers, or leave them as `NULL` to communicate its capabilities with the plugin. - report_progress: The plugin can use this to report the progress of a function that is currently running, back to the host. - report_message: The plugin can use this to communicate, errors, warnings, and other types of messages with the host. - check_cancellation: The host may desire to cancel a function call before it completes. The plugin can use this callback to check if the host requested cancellation. - report_intermediate_output: Sometiems, plugin functions that take a long time to run may choose to report intermediate data to the host. The plugin can populate one of it's output handles with intermediate data, and call this function to report it to the host."""

class OrcHost(ctypes.Structure):
    """This is how the host communicates information about itself to the plugin. This includes an optional memory allocator, various callbacks etc. The host MUST set the abi_version to that of the header used to compile the host program. - create_deck_from_proxy: If the plugin wants to create a deck by proxy, of a type that it doesn't own, it will call this function to defer to host. The host must identify which other plugin owns that datatype, and dispatch the proxy deck creation to that plugin."""

class OrcMark(ctypes.Structure):
    pass

class OrcItemProxy(ctypes.Structure):
    """Each plugin has to provide a generic way to construct decks out of a given input deck, for all of its custom datatypes. This proxy refers to a particular item in a particular deck. Both the deck and the item are referenced by their index."""

OrcDims = ctypes.c_int32 * 7

OrcPluginFunction = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.c_uint64, ctypes.POINTER(OrcHandle), ctypes.c_uint64, ctypes.POINTER(OrcHandle), ctypes.c_uint64)
OrcDeckFreeFn = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.POINTER(OrcHandle))
OrcSerializeWriteFn = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_uint64)

OrcAllocFn = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_uint64, ctypes.c_uint64)
OrcDeallocFn = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint64, ctypes.c_uint64)
OrcReportProgressFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.c_double)
OrcReportMessageFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.c_uint8, ctypes.c_char_p)
OrcCheckCancellationFn = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_uint64)
OrcReportIntermediateOutputFn = ctypes.CFUNCTYPE(None, ctypes.c_uint64, ctypes.POINTER(OrcHandle))
OrcCreateDeckFromProxyFn = ctypes.CFUNCTYPE(ctypes.c_uint32, ctypes.POINTER(OrcHandle), ctypes.c_uint64, ctypes.c_uint8, ctypes.POINTER(OrcHandle), ctypes.POINTER(OrcHandle))

OrcHandle._fields_ = [
    ("handle", ctypes.c_uint64),
    ("type_id", OrcTypeId),
    ("dims", OrcDims),
    ("n_items", ctypes.c_uint64),
    ("item_size", ctypes.c_uint64),
    ("n_marks", ctypes.c_uint64),
    ("free_fn", OrcDeckFreeFn),
    ("marks", ctypes.POINTER(OrcMark)),
    ("stride_offset", ctypes.POINTER(ctypes.c_uint64)),
    ("strides", ctypes.POINTER(ctypes.c_uint64)),
    ("items", ctypes.c_void_p),
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
    ("func", OrcPluginFunction),
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

