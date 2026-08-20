#pragma once

#include <stdbool.h>
#include <stdint.h>

// ==============================
// ABI version management
// ==============================

#define ORC_VERSION_PACK(major, minor, patch) \
  ((((uint64_t)(major)) << 42) | (((uint64_t)(minor)) << 21) | ((uint64_t)(patch)))

static const uint64_t ORC_ABI_VERSION = ORC_VERSION_PACK(0, 0, 1);

// Cross-platform symbol export. Meant for plugins to export functions that host can find
// when loading them.
#if defined(_WIN32) || defined(_WIN64)
#define ORC_PLUGIN_EXPORT __declspec(dllexport)
#else
#define ORC_PLUGIN_EXPORT __attribute__((visibility("default")))
#endif

// ==============================
// Types and Functions.
// ==============================

typedef uint64_t         OrcTypeId;
typedef uint8_t          OrcMessageLevel;
typedef uint32_t         OrcError;
typedef uint8_t          OrcProxyType;
typedef struct OrcHandle OrcHandle;

// Unsigned integers.
#define ORC_TYPE_U8 0x01u
#define ORC_TYPE_U16 0x02u
#define ORC_TYPE_U32 0x03u
#define ORC_TYPE_U64 0x04u
// Scalars.
#define ORC_TYPE_F32 0x11u
#define ORC_TYPE_F64 0x12u
// Signed integers.
#define ORC_TYPE_I8 0x21u
#define ORC_TYPE_I16 0x22u
#define ORC_TYPE_I32 0x23u
#define ORC_TYPE_I64 0x24u
// Proxy for an item in a tree.
#define ORC_TYPE_PROXY 0x30u

#define ORC_MSG_LEVEL_DEBUG 1u
#define ORC_MSG_LEVEL_INFO 2u
#define ORC_MSG_LEVEL_WARN 3u
#define ORC_MSG_LEVEL_ERROR 4u
#define ORC_MSG_LEVEL_FATAL 5u

/**
This is just a tag used to identify the types of inputs and outputs of plugin functions.
`OrcTypeId`, is the actual tag used by the host and plugin to distinguish the types.
`name` and `desc` are just optional information to help the host / user understand what
the type is.
 */
typedef struct
{
  OrcTypeId   type_id;
  char const *name;
  char const *desc;
} OrcTypeInfo;

#define ORC_ARGS_VARIADIC 0xffffffffffffffff
#define ORC_TYPE_ANY 0xffffffffffffffff

typedef OrcError (*OrcPluginFunction)(uint64_t const   ctx,
                                      OrcHandle const *inputs,
                                      uint64_t const   n_inputs,
                                      OrcHandle       *outputs,
                                      uint64_t const   n_outputs);

/**
Metadata for a function exposed by the plugin. All plugin functions have the same
signature. The metadata encodes information about the inputs, outputs, and the function
pointer.

`name` and `desc` are optional strings that are meant to help the host / user understand
what the function does. `n_inputs` and `n_outputs` can be any value, except
`ORC_ARGS_VARIADIC`. If they are set to `ORC_ARGS_VARIADIC`, host will infer that the
functions will support any number of inputs / outputs.

If `n_inputs` / `n_outputs` are set to a value other than `ORC_ARGS_VARIADIC`, then the
host can read the corresponding `input_types` and `output_types` arrays to infer the types
expected by the function. This should only be set if the function expects one concrete
type. If the function is generic / and can process many types, the corresponding type must
be set to `ORC_TYPE_ANY`. If the `input_types` and `output_types` pointers are set to
`NULL`, the host must infer that all inputs and outputs can be of any type.

This metadata is a hint for the host. The plugin function must still validate all it's
inputs, and outputs, counts, and types, and return appropriate errors. Conversely, even if
the metadata implies a certain input/output configuration is supported by the function,
the function may still fail when called with said inputs/outputs.
 */
typedef struct
{
  char const       *name;
  char const       *desc;
  uint64_t          n_inputs;
  uint64_t          n_outputs;
  OrcTypeId        *input_types;
  OrcTypeId        *output_types;
  OrcPluginFunction func;
} OrcFuncInfo;

/**
This struct is how the plugin communicates to the host about itself.

- abi_version: Must be populated with the that of the header that was used to compile the
plugin.
- name and desc: Optional information to help the host / user understand what the plugin
is about.
- types and n_types: Array of custom types that used by the functions of this plugin. It
can be `NULL` if `n_types` is zero. The plugin must not redeclare any of the primitive
types, nor can its types conflict with the types of other types loaded by other plugins.
The plugin is advised to choose a randmly generated 64 bit string as it's `OrcTypeId`, so
that it doesn't conflict with any other plugin's types.
- functions and n_functions: Array of functions exposed by this plugin. The pointer can be
`NULL` if n_types is zero.
 */
typedef struct
{
  uint64_t           abi_version;
  char const        *name;
  char const        *desc;
  OrcTypeInfo const *types;
  uint64_t           n_types;
  OrcFuncInfo const *functions;
  uint64_t           n_functions;
} OrcPlugin;

/**
Optionally the host may provide a memory allocator for the plugin. This struct is how the
host passes the function pointers to the plugin. The host may choose to leave these
`NULL`, and leave the plugin to use it's own allocator.
 */
typedef struct
{
  void *(*alloc)(uint64_t const size, uint64_t const alignment);
  void (*dealloc)(void *ptr, uint64_t const size, uint64_t const alignment);
} OrcHostMemoryAPI;

/**
The host may populate any of these function pointers, or leave them as `NULL` to
communicate its capabilities with the plugin.

- report_progress: The plugin can use this to report the progress of a function that is
currently running, back to the host.
- report_message: The plugin can use this to communicate, errors, warnings, and other
types of messages with the host.
- check_cancellation: The host may desire to cancel a function call before it completes.
The plugin can use this callback to check if the host requested cancellation.
- report_intermediate_output: Sometiems, plugin functions that take a long time to run may
choose to report intermediate data to the host. The plugin can populate one of it's output
handles with intermediate data, and call this function to report it to the host.
 */
typedef struct
{
  void (*report_progress)(uint64_t const ctx, double progress);
  void (*report_message)(uint64_t const        ctx,
                         OrcMessageLevel const level,
                         char const           *msg);
  bool (*check_cancellation)(uint64_t const ctx);
  void (*report_intermediate_output)(uint64_t const ctx, OrcHandle const *handle);
} OrcHostCallbackAPI;

/**
This is how the host communicates information about itself to the plugin. This includes an
optional memory allocator, various callbacks etc. The host MUST set the abi_version to
that of the header used to compile the host program.

- create_deck_from_proxy: If the plugin wants to create a deck by proxy, of a type that it
doesn't own, it will call this function to defer to host. The host must identify which
other plugin owns that datatype, and dispatch the proxy deck creation to that plugin.
 */
typedef struct OrcHost
{
  uint64_t           abi_version;
  OrcHostMemoryAPI   memory_api;
  OrcHostCallbackAPI callbacks;

  OrcError (*create_deck_from_proxy)(OrcHandle const   *inputs,
                                     uint64_t const     n_inputs,
                                     OrcProxyType const proxy_type,
                                     OrcHandle const   *proxy,
                                     OrcHandle         *out);
} OrcHost;

/**
The ABI optionally supports attaching units to the inputs and outputs of the plugin
functions. These are completely optional, and it is up to each plugin function / and each
host whether they're respected, checked or used in anyway.
 */
#define ORC_DIM_LENGTH 0u
#define ORC_DIM_MASS 1u
#define ORC_DIM_TIME 2u
#define ORC_DIM_ELECTRIC_CURRENT 3u
#define ORC_DIM_TEMPERATURE 4u
#define ORC_DIM_SUBSTANCE 5u
#define ORC_DIM_LUMINOSITY 6u

#define ORC_NUM_DIMS 7u

typedef int32_t OrcDims[ORC_NUM_DIMS];

typedef struct
{
  uint8_t  depth;
  uint64_t pos;
} OrcMark;

/// Error codes.
#define ORC_ERROR_NONE 0u
#define ORC_ERROR_ABI_VERSION_MISMATCH 0xff01u
#define ORC_ERROR_INVALID_HANDLE 0xff02u
#define ORC_ERROR_INVALID_DIMENSIONS 0xff03u
#define ORC_ERROR_TYPE_MISMATCH 0xff04u
#define ORC_ERROR_INVALID_COMBINATIONS 0xff05u
#define ORC_ERROR_PLUGIN_ALREADY_INITIALIZED 0xff06u
#define ORC_ERROR_CONCURRENCY_PROBLEM 0xff07u
#define ORC_ERROR_INVALID_PROXY 0xff08u
#define ORC_ERROR_CANNOT_LOAD_PLUGINS 0xff09u
#define ORC_ERROR_OUT_OF_BOUNDS 0xff0au
#define ORC_ERROR_ALLOC_FAILED 0xff0bu
#define ORC_ERROR_NULL_PTR 0xff0cu
#define ORC_ERROR_MISSING_CAPABILITY 0xff0du
#define ORC_ERROR_INVALID_FUNCTION 0xff0eu
#define ORC_ERROR_INVALID_ARGUMENTS 0xff0fu
#define ORC_ERROR_SERIALIZATION_ERROR 0xff10u
#define ORC_ERROR_UNKNOWN 0xffffu

/**
Various types of proxy decks.

- COPY_ALL: This is only valid for a single input deck. All the contents of the input deck
will be copied.
- COPY_ITEMS: This is only valid for a single input deck. All the items of the input deck
will be copied into the output deck, but the structure of the deck will be copied from the
proxy deck.
- SHUFFLE: This supports more than one input decks. The output deck will be populated by
items that are referenced by the proxy deck via `OrcItemProxy`. The structure of the deck
will match that of the proxy deck.

All input decks must be of the same type, and hence have the same type_id. This will be
copied into the output deck. Other metadata such as `dims` are copied from the proxy deck.

 */
#define ORC_DECK_PROXY_COPY_ALL 0x01u
#define ORC_DECK_PROXY_COPY_ITEMS 0x02u
#define ORC_DECK_PROXY_SHUFFLE 0x03u

/**
In C++ vocabulary, this is the destructor of the data behind an `OrcHandle`. When said
data doesn't need a destructor, say, when it is allocated directly on the stack, this can
be left `NULL` inside `OrcHandle`.

The plugin or the host may call this function when they want to free the data, and use the
same handle to allocate some other data.
 */
typedef OrcError (*OrcDeckFreeFn)(OrcHandle *const handle);

/**
This is the primary handle passed around between hosts and plugins as input and outputs
for the plugin function calls.

- handle: This is a unique integer assigned by the host program. The plugin MUST NOT
modify it. The plugin may use it as a key to point to data allocated within it's memory,
when said data is owned by the same plugin. The host uses this integer to keep track of
all the data alive during a session.
- items, and n_items: These are the actual data stored in the deck that this handles
points to. This handle can be backed by any implementation of a Deck datastructure,
written in any language, as long as these pointers meet the ABI requirements to be read
across the FFI boundary.
- marks, stride_offset, and n_mmarks: These define the structure / nesting inside a deck.
- type_id: Indicates the type of the data stored in the deck backs this handle.
- dims: Optional units. Just metadata, the plugin functions and host can do whatever they
want with it.
- free_fn: Destructor. Whichever plugin allocates the backing deck, and populates this
handle with the corresponding pointers, is also responsible for setting the destructor
function pointer. Without this, the memory may leak.
 */
struct OrcHandle
{
  // Assigned by the host before lending this handle to any plugin.  Never modified after
  // assignment, even when the backing data is freed.  Plugins use this as the key into
  // their internal registry.
  uint64_t        handle;
  OrcTypeId       type_id;
  OrcDims         dims;
  uint64_t        n_items;
  uint64_t        item_size;
  uint64_t        n_marks;
  OrcDeckFreeFn   free_fn;
  OrcMark const  *marks;
  uint64_t const *stride_offset;
  uint64_t const *strides;
  void const     *items;
};

/**
Each plugin has to provide a generic way to construct decks out of a given input deck, for
all of its custom datatypes. This proxy refers to a particular item in a particular deck.
Both the deck and the item are referenced by their index.
 */
typedef struct
{
  uint64_t tree;
  uint64_t item;
} OrcItemProxy;

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

/**
The host will call this plugin when it loads a plugin. It must communicate it's own
capabilities via the host argument, and let the plugin populate it's own information into
teh `plugin_data_out` argument.
 */
ORC_PLUGIN_EXPORT OrcError orc_plugin_init(OrcHost const *host,
                                           OrcPlugin     *plugin_data_out);

/**
The host may call this function to allocate a deck for this handle. The plugin must
respect the `out->handle` property. It must not be modified, and must be used as a key to
point to the allocated data, until `orc_deck_free` is called, with the same `handle`
property.
 */
ORC_PLUGIN_EXPORT OrcError orc_deck_alloc(OrcTypeId const id, OrcHandle *const out);

ORC_PLUGIN_EXPORT OrcError orc_deck_free(OrcHandle *const handle);

ORC_PLUGIN_EXPORT OrcError orc_deck_from_proxy(OrcHandle const   *inputs,
                                               uint64_t const     n_inputs,
                                               OrcProxyType const proxy_type,
                                               OrcHandle const   *proxy,
                                               OrcHandle         *out);

typedef OrcError (*OrcSerializeWriteFn)(uint64_t const ctx,
                                        void const    *data,
                                        uint64_t const len);

ORC_PLUGIN_EXPORT OrcError orc_deck_serialize_items(uint64_t const      ctx,
                                                    OrcHandle const    *handle,
                                                    OrcSerializeWriteFn write_fn);

ORC_PLUGIN_EXPORT OrcError orc_deck_deserialize_items(uint64_t const  ctx,
                                                      OrcTypeId const type_id,
                                                      void const     *buf,
                                                      uint64_t const  buf_len,
                                                      OrcHandle      *out);
