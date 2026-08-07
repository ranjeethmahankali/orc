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

typedef struct
{
  OrcTypeId   type_id;
  char const *name;
  char const *desc;
} OrcTypeInfo;

#define ORC_ARGS_VARIADIC 0xffffffffffffffff
#define ORC_TYPE_ANY 0xffffffffffffffff

typedef struct
{
  char const *name;
  char const *desc;
  uint64_t    n_inputs;
  uint64_t    n_outputs;
  OrcTypeId  *input_types;
  OrcTypeId  *output_types;

  void (*func)(uint64_t const   ctx,
               OrcHandle const *inputs,
               uint64_t const   n_inputs,
               OrcHandle       *outputs,
               uint64_t const   n_outputs);
} OrcFuncInfo;

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

typedef struct
{
  void *(*alloc)(uint64_t const size, uint64_t const alignment);
  void (*dealloc)(void *ptr, uint64_t const size, uint64_t const alignment);
} OrcHostMemoryAPI;

typedef struct
{
  void (*report_progress)(uint64_t const ctx, double progress);
  void (*report_message)(uint64_t const        ctx,
                         OrcMessageLevel const level,
                         char const           *msg);
  bool (*check_cancellation)(uint64_t const ctx);
  void (*report_intermediate_output)(uint64_t const ctx, OrcHandle const *handle);
} OrcHostCallbackAPI;

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
#define ORC_ERROR_UNKNOWN 0xffffu

#define ORC_DECK_PROXY_COPY_ALL 0x01u
#define ORC_DECK_PROXY_COPY_ITEMS 0x02u
#define ORC_DECK_PROXY_SHUFFLE 0x03u

typedef OrcError (*OrcDeckFreeFn)(OrcHandle *const handle);

struct OrcHandle
{
  // Assigned by the host before lending this handle to any plugin.  Never modified after
  // assignment, even when the backing data is freed.  Plugins use this as the key into
  // their internal registry.
  uint64_t        handle;
  void const     *items;
  uint64_t        n_items;
  uint64_t        item_size;
  OrcMark const  *marks;
  uint64_t const *stride_offset;
  uint64_t        n_marks;
  uint64_t const *strides;
  OrcTypeId       type_id;
  OrcDims         dims;
  OrcDeckFreeFn   free_fn;
};

// Each plugin has to provide a generic way to construct decks out of a given input deck,
// for all of its custom datatypes.
typedef struct
{
  uint64_t tree;
  uint64_t item;
} OrcItemProxy;

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

// Loading the plugin, and register the host with the plugin.
ORC_PLUGIN_EXPORT OrcError orc_plugin_init(OrcHost const *host,
                                           OrcPlugin     *plugin_data_out);

ORC_PLUGIN_EXPORT OrcError orc_deck_alloc(OrcTypeId const id, OrcHandle *const out);

ORC_PLUGIN_EXPORT OrcError orc_deck_free(OrcHandle *const handle);

ORC_PLUGIN_EXPORT OrcError orc_deck_from_proxy(OrcHandle const   *inputs,
                                               uint64_t const     n_inputs,
                                               OrcProxyType const proxy_type,
                                               OrcHandle const   *proxy,
                                               OrcHandle         *out);
