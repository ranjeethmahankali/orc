#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// ==============================
// ABI version management
// ==============================

#define ORC_VERSION_PACK(major, minor, patch) \
  ((((uint64_t)(major)) << 42) | (((uint64_t)(minor)) << 21) | ((uint64_t)(patch)))

static const uint64_t ORC_ABI_VERSION = ORC_VERSION_PACK(0, 0, 1);

// ==============================
// Types and Functions.
// ==============================

typedef uint64_t OrcTypeId;

// Unsigned integers.
#define ORC_TYPE_U8 0x01
#define ORC_TYPE_U16 0x02
#define ORC_TYPE_U32 0x03
#define ORC_TYPE_U64 0x04
// Scalars.
#define ORC_TYPE_F32 0x11
#define ORC_TYPE_F64 0x12
// Signed integers.
#define ORC_TYPE_I8 0x21
#define ORC_TYPE_I16 0x22
#define ORC_TYPE_I32 0x23
#define ORC_TYPE_I64 0x24
// Proxy for an item in a tree.
#define ORC_TYPE_PROXY 0x30

// Message levels
typedef uint8_t OrcMessageLevel;

#define ORC_MSG_LEVEL_DEBUG 1
#define ORC_MSG_LEVEL_INFO 2
#define ORC_MSG_LEVEL_WARN 3
#define ORC_MSG_LEVEL_ERROR 4
#define ORC_MSG_LEVEL_FATAL 5

typedef struct OrcHandle OrcHandle;

typedef struct
{
  OrcTypeId   type_id;
  char const *name;
  char const *desc;
} OrcTypeInfo;

typedef struct
{
  char const *name;
  char const *desc;

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
} OrcHost;

#define ORC_DIM_LENGTH 0u
#define ORC_DIM_MASS 1u
#define ORC_DIM_TIME 2u
#define ORC_DIM_CURRENT 3u
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

struct OrcHandle
{
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
};

// Each plugin has to provide a generic way to construct decks out of a given input deck,
// for all of its custom datatypes.
typedef struct
{
  uint64_t tree;
  uint64_t item;
} OrcItemProxy;

typedef uint8_t OrcProxyType;

#define ORC_DECK_PROXY_COPY_ALL 0x01
#define ORC_DECK_PROXY_COPY_ITEMS 0x02
#define ORC_DECK_PROXY_SHUFFLE 0x03

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

typedef uint32_t OrcError;

#define ORC_ERROR_NONE 0
#define ORC_ERROR_ABI_VERSION_MISMATCH 0xff01
#define ORC_ERROR_INVALID_HANDLE 0xff02
#define ORC_ERROR_INVALID_DIMENSIONS 0xff03
#define ORC_ERROR_TYPE_MISMATCH 0xff04
#define ORC_ERROR_INVALID_COMBINATIONS 0xff05
#define ORC_ERROR_PLUGIN_ALREADY_INITIALIZED 0xff06
#define ORC_ERROR_CONCURRENCY_PROBLEM 0xff07
#define ORC_ERROR_INVALID_PROXY 0xff08
#define ORC_ERROR_UNKNOWN 0xffff

// Loading the plugin, and register the host with the plugin.
OrcError orc_plugin_init(OrcHost const *host, OrcPlugin *plugin_data_out);

// Deck lifetime operations.
OrcError orc_deck_alloc(OrcTypeId const id, OrcHandle *const out);
OrcError orc_deck_free(OrcHandle *const handle);

OrcError orc_deck_from_proxy(OrcHandle const   *inputs,
                             uint64_t const     n_inputs,
                             OrcProxyType const proxy_type,
                             OrcHandle const   *proxy,
                             OrcHandle         *out);
