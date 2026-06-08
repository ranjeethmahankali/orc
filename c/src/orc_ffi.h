#include <stddef.h>
#include <stdint.h>

// Primitive type ids.
// Unsigned integers.
#define ORC_U8 ((uint32_t)0x00)
#define ORC_U16 ((uint32_t)0x01)
#define ORC_U32 ((uint32_t)0x02)
#define ORC_U64 ((uint32_t)0x03)
// Scalars.
#define ORC_F32 ((uint32_t)0x04)
#define ORC_F64 ((uint32_t)0x05)
// Signed integers.
#define ORC_I8 ((uint32_t)0x10)
#define ORC_I16 ((uint32_t)0x11)
#define ORC_I32 ((uint32_t)0x12)
#define ORC_I64 ((uint32_t)0x13)
// All custom opaque types defined by a plugin.
#define ORC_OPAQUE ((uint32_t)UINT32_MAX)

typedef struct
{
  uint32_t primitive_id;
  uint32_t opaque_id;
} OrcTypeId;

typedef struct OrcHandle OrcHandle;

typedef struct
{
  OrcTypeId type_id;
  char*     name;
  char*     desc;

  void (*render_fn)(uint64_t const ctx, OrcHandle const* input);
  void (*string_fn)(uint64_t const ctx, OrcHandle const* input, OrcHandle* out_str);
  void (*clone_fn)(uint64_t const ctx, void* src, void* dst);
} OrcTypeInfo;

typedef struct
{
  char*      name;
  char*      desc;
  OrcTypeId* inputs;
  OrcTypeId* outputs;
} OrcFuncInfo;

typedef struct
{
  OrcTypeInfo* types;
  uint64_t     n_types;
  OrcFuncInfo* functions;
  uint64_t     n_functions;
} OrcPlugin;

typedef struct OrcHost
{
  struct
  {
    void* (*alloc)(size_t const nbytes);
    void* (*realloc)(void* ptr, size_t const nbytes);
    void (*free)(void* ptr);
  } memory_api;
  struct
  {
    void (*draw_points)(void* data);
    void (*draw_lines)(void* data);
    void (*draw_triangles)(void* data);
  } rendering_api;  // This is just to get the idea. I don't know what the rendering API
                    // should look like.
  struct
  {
    void (*report_progress)(uint64_t const ctx, double progress);
    void (*report_error)(uint64_t const ctx, char* error);
    void (*report_warning)(uint64_t const ctx, char* warning);
    bool (*check_cancellation)(uint64_t const ctx);
  } callbacks;
} OrcHost;

void orc_plugin_init(OrcHost const* host, OrcPlugin* plugin_data_out);

void orc_plugin_data_free(OrcPlugin* plugin_data);

// ========== Units ==========

#define ORC_DIM_LENGTH ((uint32_t)0)
#define ORC_DIM_MASS ((uint32_t)1)
#define ORC_DIM_TIME ((uint32_t)2)
#define ORC_DIM_CURRENT ((uint32_t)3)
#define ORC_DIM_TEMPERATURE ((uint32_t)4)
#define ORC_DIM_SUBSTANCE ((uint32_t)5)
#define ORC_DIM_LUMINOSITY ((uint32_t)6)

#define ORC_NUM_DIMS 7

typedef int32_t Dims[ORC_NUM_DIMS];

typedef struct
{
  uint8_t  depth;
  uint64_t pos;
} OrcMark;

struct OrcHandle
{
  uint64_t    handle;
  void*       items;
  uint64_t    n_items;
  uint64_t    item_size;
  OrcMark*    marks;
  uint64_t*   stride_offset;
  uint64_t    n_marks;
  uint64_t*   strides;
  OrcTypeInfo type_info;
};

OrcTypeInfo _orc_type_info_u8(void);
OrcTypeInfo _orc_type_info_u16(void);
OrcTypeInfo _orc_type_info_u32(void);
OrcTypeInfo _orc_type_info_u64(void);
OrcTypeInfo _orc_type_info_f32(void);
OrcTypeInfo _orc_type_info_f64(void);
OrcTypeInfo _orc_type_info_i8(void);
OrcTypeInfo _orc_type_info_i16(void);
OrcTypeInfo _orc_type_info_i32(void);
OrcTypeInfo _orc_type_info_i64(void);

void orc_deck_alloc(OrcTypeId const id, OrcHandle* const out);

void orc_deck_free(OrcHandle* const handle);
