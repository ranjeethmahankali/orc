#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

// Unsigned integers.
#define ORC_U8 0x01u
#define ORC_U16 0x02u
#define ORC_U32 0x03u
#define ORC_U64 0x04u
// Scalars.
#define ORC_F32 0x05u
#define ORC_F64 0x06u
// Signed integers.
#define ORC_I8 0x11u
#define ORC_I16 0x12u
#define ORC_I32 0x13u
#define ORC_I64 0x14u
// Proxy for an item in a tree.
#define ORC_PROXY 0x40u
// All custom opaque types defined by a plugin.
#define ORC_OPAQUE 0xffffffffu

typedef struct
{
  uint32_t primitive_id;
  uint32_t opaque_id;
} OrcTypeId;

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
  uint64_t    n_inputs;
  uint64_t    n_outputs;

  void (*func)(uint64_t const   ctx,
               OrcHandle const *inputs,
               size_t const     n_inputs,
               OrcHandle       *outputs,
               size_t const     n_outputs);
} OrcFuncInfo;

typedef struct
{
  OrcTypeInfo const *types;
  uint64_t           n_types;
  OrcFuncInfo const *functions;
  uint64_t           n_functions;
} OrcPlugin;

typedef struct OrcHost
{
  struct
  {
    void *(*alloc)(uint64_t const size, uint64_t const alignment);
    void (*dealloc)(void *ptr, uint64_t const size, uint64_t const alignment);
  } memory_api;

  struct
  {
    void (*report_progress)(uint64_t const ctx, double progress);
    void (*report_error)(uint64_t const ctx, char const *error);
    void (*report_warning)(uint64_t const ctx, char const *warning);
    bool (*check_cancellation)(uint64_t const ctx);
  } callbacks;
} OrcHost;

#define ORC_DIM_LENGTH ((uint32_t)0)
#define ORC_DIM_MASS ((uint32_t)1)
#define ORC_DIM_TIME ((uint32_t)2)
#define ORC_DIM_CURRENT ((uint32_t)3)
#define ORC_DIM_TEMPERATURE ((uint32_t)4)
#define ORC_DIM_SUBSTANCE ((uint32_t)5)
#define ORC_DIM_LUMINOSITY ((uint32_t)6)

#define ORC_NUM_DIMS 7

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
// for all of it's custom datatypes.
typedef struct
{
  uint64_t tree;
  uint64_t item;
} OrcItemProxy;

#define ORC_DECK_PROXY_COPY_ALL ((uint32_t)0x01)
#define ORC_DECK_PROXY_COPY_ITEMS ((uint32_t)0x02)
#define ORC_DECK_PROXY_SHUFFLE ((uint32_t)0x03)

// ===========================================================
// Functions meant to be implemented by the plugin.
// ===========================================================

// Loading the plugin, and register the host with the plugin.
void orc_plugin_init(OrcHost const *host, OrcPlugin *plugin_data_out);

// Deck lifetime operations.
void orc_deck_alloc(OrcTypeId const id, OrcHandle *const out);
void orc_deck_free(OrcHandle *const handle);

void orc_deck_from_proxy(OrcHandle const *inputs,
                         uint64_t const   n_inputs,
                         uint32_t const   proxy_type,
                         OrcHandle const *proxy,
                         OrcHandle       *out);
