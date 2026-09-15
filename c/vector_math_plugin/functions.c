#include <math.h>
#include <orc_abi.h>
#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include <string.h>

static bool is_primitive_type(OrcTypeId const type_id)
{
  switch (type_id) {
  case ORC_TYPE_U8:   // Fall through.
  case ORC_TYPE_U16:  // Fall through.
  case ORC_TYPE_U32:  // Fall through.
  case ORC_TYPE_U64:  // Fall through.
  case ORC_TYPE_F32:  // Fall through.
  case ORC_TYPE_F64:  // Fall through.
  case ORC_TYPE_I8:   // Fall through.
  case ORC_TYPE_I16:  // Fall through.
  case ORC_TYPE_I32:  // Fall through.
  case ORC_TYPE_I64:  // Fall through.
    return true;
  default:
    return false;
  }
}

static bool is_float_type(OrcTypeId const type_id)
{
  return type_id == ORC_TYPE_F32 || type_id == ORC_TYPE_F64;
}

static OrcError make_vec(uint64_t         ctx,
                         OrcHandle const *input,
                         uint64_t         n_inputs,
                         OrcHandle       *output,
                         uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs < 2) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Need at least two components to create a vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_primitive_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Vector components must be primitive scalar types");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  OrcError          err           = ORC_ERROR_NONE;
  size_t           *input_arities = NULL;
  void             *combinations  = NULL;
  OrcHandle const **input_ptrs    = NULL;
  uint8_t          *input_depths  = NULL;
  // Stuff above need to be cleaned up in all exit paths.
  size_t output_arity = 0;
  size_t scalar_size  = 0;
  {  // Make sure all the inputs (components) are the same type. And compute the output
     // arity at the same time.
    for (size_t i = 0; i < n_inputs; ++i) {
      if (input[i].type_id != first_type_id) {
        orc_sdk_report_message(ctx,
                               ORC_MSG_LEVEL_ERROR,
                               "All components of the vector must of of the same type.");
        err = ORC_ERROR_INVALID_ARGUMENTS;
        goto cleanup;
      }
      OrcSdk_TypeInfo type_info = {0};
      err                       = orc_sdk_get_type_info(input[i].type_id, &type_info);
      if (err)
        goto cleanup;
      if (scalar_size == 0) {
        scalar_size = type_info.item_size;
      }
      else {  // Already assigned, just assert it is the same.
        ORC_SDK_REQUIRE_WITH_MSG(
          scalar_size == type_info.item_size,
          "This is not an error, but an assert instead because this implies a bug in our "
          "SDK, and should never happen.");
      }
      if (input[i].item_size == 0) {
        orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
        err = ORC_ERROR_INVALID_HANDLE;
        goto cleanup;
      }
      if (input[i].item_size % scalar_size) {
        // The size of the aggregate type must be a multiple of the scalar size.
        orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
        err = ORC_ERROR_INVALID_ARGUMENTS;
        goto cleanup;
      }
      size_t const arity = input[i].item_size / scalar_size;
      err                = orc_sdk_arr_push(input_arities, arity);
      if (err)
        goto cleanup;
      output_arity += arity;
    }
  }
  // Allocate output.
  err = orc_sdk_handle_alloc(first_type_id, scalar_size * output_arity, output);
  if (err)
    goto cleanup;
  // Stride over inputs with list combinations, and assign the output.
  {
    // Input depths array.
    orc_sdk_arr_resize(input_depths, n_inputs);
    uint8_t const zero_depth = 0;
    orc_sdk_arr_fill(input_depths, zero_depth);
    // Pack input handle pointers into an array.
    orc_sdk_arr_clear(input_ptrs);
    orc_sdk_arr_reserve(input_ptrs, n_inputs);
    for (size_t i = 0; i < n_inputs; ++i) {
      err = orc_sdk_arr_push(input_ptrs, input + i);
      if (err)
        goto cleanup;
    }
    // Check the outputs and initialize the combinations.
    ORC_SDK_REQUIRE_WITH_MSG(
      n_outputs == 1,
      "We already checked before. This is just to make sure we don't go out of sync.");
    combinations = orc_sdk_comb_init(
      input_ptrs, input_depths, n_inputs, &output, (uint8_t const[]) {0}, 1);
  }
  if (combinations == NULL) {
    err = ORC_ERROR_INVALID_COMBINATIONS;
    goto cleanup;
  }
  while (combinations) {
    OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);
    // Casting to char* so that I can increment by byte.
    char *out_vec = (char *)orc_sdk_dw_push_empty(out_writer);
    if (out_vec == NULL) {
      err = ORC_ERROR_ALLOC_FAILED;
      goto cleanup;
    }
    for (size_t i = 0; i < n_inputs; ++i) {
      OrcSdk_DeckView in_view    = orc_sdk_comb_get_input(combinations, i);
      void const     *in_vec     = orc_sdk_dv_item_ptr(&in_view);
      size_t const    input_size = input_arities[i] * scalar_size;
      memcpy(out_vec, in_vec, input_size);
      out_vec += input_size;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_oh_update(output);
cleanup:
  orc_sdk_comb_free(combinations);
  orc_sdk_arr_free(input_arities);
  orc_sdk_arr_free(input_ptrs);
  orc_sdk_arr_free(input_depths);
  return err;
}

OrcFuncInfo const MAKE_VEC_INFO = {
  .name = "make_vec",
  .desc =
    "Create a vector from its components. The arity of the output will be the sum of "
    "arities of inputs. Supports any scalar type.",
  .n_inputs    = ORC_ARGS_VARIADIC,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = make_vec};

// One type-specific implementation per primitive scalar type. Each owns the whole
// combinations loop, with pointers already cast to `type`, so vec_add only needs to
// pick one of these once instead of switching on the type for every item.
#define DEFINE_VEC_ADD_DISPATCH(type, suffix)                                      \
  static OrcError _vec_add_##suffix(                                               \
    void *combinations, uint64_t const n_inputs, size_t const arity)               \
  {                                                                                \
    while (combinations) {                                                         \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);    \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer);   \
      if (out_vec == NULL) {                                                       \
        orc_sdk_comb_free(combinations);                                           \
        return ORC_ERROR_ALLOC_FAILED;                                             \
      }                                                                            \
      OrcSdk_DeckView first_view = orc_sdk_comb_get_input(combinations, 0);        \
      type const     *first_vec  = (type const *)orc_sdk_dv_item_ptr(&first_view); \
      for (size_t j = 0; j < arity; ++j) {                                         \
        out_vec[j] = first_vec[j];                                                 \
      }                                                                            \
      for (uint64_t i = 1; i < n_inputs; ++i) {                                    \
        OrcSdk_DeckView in_view = orc_sdk_comb_get_input(combinations, i);         \
        type const     *in_vec  = (type const *)orc_sdk_dv_item_ptr(&in_view);     \
        for (size_t j = 0; j < arity; ++j) {                                       \
          out_vec[j] = (type)(out_vec[j] + in_vec[j]);                             \
        }                                                                          \
      }                                                                            \
      combinations = orc_sdk_comb_advance(combinations);                           \
    }                                                                              \
    return ORC_ERROR_NONE;                                                         \
  }

DEFINE_VEC_ADD_DISPATCH(uint8_t, u8)
DEFINE_VEC_ADD_DISPATCH(uint16_t, u16)
DEFINE_VEC_ADD_DISPATCH(uint32_t, u32)
DEFINE_VEC_ADD_DISPATCH(uint64_t, u64)
DEFINE_VEC_ADD_DISPATCH(float, f32)
DEFINE_VEC_ADD_DISPATCH(double, f64)
DEFINE_VEC_ADD_DISPATCH(int8_t, i8)
DEFINE_VEC_ADD_DISPATCH(int16_t, i16)
DEFINE_VEC_ADD_DISPATCH(int32_t, i32)
DEFINE_VEC_ADD_DISPATCH(int64_t, i64)

static OrcError vec_add(uint64_t         ctx,
                        OrcHandle const *input,
                        uint64_t         n_inputs,
                        OrcHandle       *output,
                        uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need at least two vectors to add.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_primitive_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Vector components must be primitive scalar types");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  // Unlike make_vec, addition is elementwise -- every input must share the exact same
  // type and arity, so there is no need for a per-input arities array here.
  for (uint64_t i = 1; i < n_inputs; ++i) {
    if (input[i].type_id != first_type_id) {
      orc_sdk_report_message(
        ctx, ORC_MSG_LEVEL_ERROR, "All input vectors must be of the same type.");
      return ORC_ERROR_INVALID_ARGUMENTS;
    }
    if (input[i].item_size != first_item_size) {
      orc_sdk_report_message(
        ctx, ORC_MSG_LEVEL_ERROR, "All input vectors must be of the same arity.");
      return ORC_ERROR_INVALID_ARGUMENTS;
    }
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  // Allocate output.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  void             *combinations = NULL;
  OrcHandle const **input_ptrs   = NULL;
  uint8_t          *input_depths = NULL;
  // Above three need to be cleaned up in all exit paths.
  {
    // Input depths array.
    orc_sdk_arr_resize(input_depths, n_inputs);
    uint8_t const zero_depth = 0;
    orc_sdk_arr_fill(input_depths, zero_depth);
    // Pack input handle pointers into an array.
    orc_sdk_arr_reserve(input_ptrs, n_inputs);
    for (uint64_t i = 0; i < n_inputs; ++i) {
      err = orc_sdk_arr_push(input_ptrs, input + i);
      if (err)
        goto cleanup;
    }
    // Check the outputs and initialize the combinations.
    ORC_SDK_REQUIRE_WITH_MSG(
      n_outputs == 1,
      "We already checked before. This is just to make sure we don't go out of sync.");
    combinations = orc_sdk_comb_init(
      input_ptrs, input_depths, n_inputs, &output, (uint8_t const[]) {0}, 1);
  }
  if (combinations == NULL) {
    err = ORC_ERROR_INVALID_COMBINATIONS;
    goto cleanup;
  }
  // Dispatch once on the component type here, instead of switching on it for every item
  // inside the combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_U8:
    err = _vec_add_u8(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_U16:
    err = _vec_add_u16(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_U32:
    err = _vec_add_u32(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_U64:
    err = _vec_add_u64(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_F32:
    err = _vec_add_f32(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_add_f64(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_I8:
    err = _vec_add_i8(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_I16:
    err = _vec_add_i16(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_I32:
    err = _vec_add_i32(combinations, n_inputs, arity);
    break;
  case ORC_TYPE_I64:
    err = _vec_add_i64(combinations, n_inputs, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_primitive_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error. Either way it must not be touched or freed again below.
  combinations = NULL;
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
cleanup:
  orc_sdk_comb_free(combinations);
  orc_sdk_arr_free(input_ptrs);
  orc_sdk_arr_free(input_depths);
  return err;
}

OrcFuncInfo const VEC_ADD_INFO = {
  .name = "vec_add",
  .desc =
    "Add two or more vectors component wise. Works for vectors of any arity, and any "
    "primitive scalar type. All the input vectors must be of the same scalar type and "
    "arity. E.g. I cannot add dvec3 with vec2.",
  .n_inputs    = ORC_ARGS_VARIADIC,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_add};

// One type-specific implementation per primitive scalar type. Each owns the whole
// combinations loop, with pointers already cast to `type`, so vec_subtract only needs to
// pick one of these once instead of switching on the type for every item. Unlike
// vec_add's dispatch, there is no n_inputs fold here: subtraction is order-sensitive and
// this function's arity is fixed at exactly 2 (see VEC_SUBTRACT_INFO.n_inputs), so we
// always read input 0 and input 1 directly.
#define DEFINE_VEC_SUBTRACT_DISPATCH(type, suffix)                               \
  static OrcError _vec_subtract_##suffix(void *combinations, size_t const arity) \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_vec == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView a_view = orc_sdk_comb_get_input(combinations, 0);          \
      OrcSdk_DeckView b_view = orc_sdk_comb_get_input(combinations, 1);          \
      type const     *a_vec  = (type const *)orc_sdk_dv_item_ptr(&a_view);       \
      type const     *b_vec  = (type const *)orc_sdk_dv_item_ptr(&b_view);       \
      for (size_t j = 0; j < arity; ++j) {                                       \
        out_vec[j] = (type)(a_vec[j] - b_vec[j]);                                \
      }                                                                          \
      combinations = orc_sdk_comb_advance(combinations);                         \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_VEC_SUBTRACT_DISPATCH(uint8_t, u8)
DEFINE_VEC_SUBTRACT_DISPATCH(uint16_t, u16)
DEFINE_VEC_SUBTRACT_DISPATCH(uint32_t, u32)
DEFINE_VEC_SUBTRACT_DISPATCH(uint64_t, u64)
DEFINE_VEC_SUBTRACT_DISPATCH(float, f32)
DEFINE_VEC_SUBTRACT_DISPATCH(double, f64)
DEFINE_VEC_SUBTRACT_DISPATCH(int8_t, i8)
DEFINE_VEC_SUBTRACT_DISPATCH(int16_t, i16)
DEFINE_VEC_SUBTRACT_DISPATCH(int32_t, i32)
DEFINE_VEC_SUBTRACT_DISPATCH(int64_t, i64)

static OrcError vec_subtract(uint64_t         ctx,
                             OrcHandle const *input,
                             uint64_t         n_inputs,
                             OrcHandle       *output,
                             uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 2) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Need exactly two vectors to subtract.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_primitive_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Vector components must be primitive scalar types");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  // Subtraction is elementwise -- both inputs must share the exact same type and arity.
  if (input[1].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[1].item_size != first_item_size) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same arity.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  // Allocate output.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 2, so the input pointers/depths can just live on the stack --
  // no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {input + 0, input + 1},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         &output,
                                         (uint8_t const[]) {0},
                                         1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of switching on it for every item
  // inside the combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_U8:
    err = _vec_subtract_u8(combinations, arity);
    break;
  case ORC_TYPE_U16:
    err = _vec_subtract_u16(combinations, arity);
    break;
  case ORC_TYPE_U32:
    err = _vec_subtract_u32(combinations, arity);
    break;
  case ORC_TYPE_U64:
    err = _vec_subtract_u64(combinations, arity);
    break;
  case ORC_TYPE_F32:
    err = _vec_subtract_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_subtract_f64(combinations, arity);
    break;
  case ORC_TYPE_I8:
    err = _vec_subtract_i8(combinations, arity);
    break;
  case ORC_TYPE_I16:
    err = _vec_subtract_i16(combinations, arity);
    break;
  case ORC_TYPE_I32:
    err = _vec_subtract_i32(combinations, arity);
    break;
  case ORC_TYPE_I64:
    err = _vec_subtract_i64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_primitive_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_SUBTRACT_INFO = {
  .name = "vec_subtract",
  .desc =
    "Subtract the second vector from the first. Supports vectors of any arity, and any "
    "primitive scalar type. Both input vectors must be of the same scalar type and "
    "arity.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_subtract};

// One type-specific implementation per supported scalar type -- only float and double,
// unlike vec_add/vec_subtract's full set of primitives. Each owns the whole combinations
// loop, with pointers already cast to `type`, so vec_dot_product only needs to pick one
// of these once instead of switching on the type for every item. The output here is a
// single scalar (arity 1), not a vector of the same arity as the inputs -- it's a
// reduction, not an elementwise op.
#define DEFINE_VEC_DOT_PRODUCT_DISPATCH(type, suffix)                               \
  static OrcError _vec_dot_product_##suffix(void *combinations, size_t const arity) \
  {                                                                                 \
    while (combinations) {                                                          \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);     \
      type              *out_val    = (type *)orc_sdk_dw_push_empty(out_writer);    \
      if (out_val == NULL) {                                                        \
        orc_sdk_comb_free(combinations);                                            \
        return ORC_ERROR_ALLOC_FAILED;                                              \
      }                                                                             \
      OrcSdk_DeckView a_view = orc_sdk_comb_get_input(combinations, 0);             \
      OrcSdk_DeckView b_view = orc_sdk_comb_get_input(combinations, 1);             \
      type const     *a_vec  = (type const *)orc_sdk_dv_item_ptr(&a_view);          \
      type const     *b_vec  = (type const *)orc_sdk_dv_item_ptr(&b_view);          \
      type            sum    = 0;                                                   \
      for (size_t j = 0; j < arity; ++j) {                                          \
        sum = (type)(sum + a_vec[j] * b_vec[j]);                                    \
      }                                                                             \
      *out_val     = sum;                                                           \
      combinations = orc_sdk_comb_advance(combinations);                            \
    }                                                                               \
    return ORC_ERROR_NONE;                                                          \
  }

DEFINE_VEC_DOT_PRODUCT_DISPATCH(float, f32)
DEFINE_VEC_DOT_PRODUCT_DISPATCH(double, f64)

static OrcError vec_dot_product(uint64_t         ctx,
                                OrcHandle const *input,
                                uint64_t         n_inputs,
                                OrcHandle       *output,
                                uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 2) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Need exactly two vectors for a dot product.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Dot product only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  // The dot product is a reduction over matching components -- both inputs must share
  // the exact same type and arity.
  if (input[1].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[1].item_size != first_item_size) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same arity.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  // Allocate output -- a single scalar, not a vector of the input arity.
  err = orc_sdk_handle_alloc(first_type_id, scalar_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 2, so the input pointers/depths can just live on the stack --
  // no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {input + 0, input + 1},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         &output,
                                         (uint8_t const[]) {0},
                                         1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of switching on it for every item
  // inside the combinations loop. Only float and double are supported.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_dot_product_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_dot_product_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_DOT_PRODUCT_INFO = {
  .name = "vec_dot_product",
  .desc =
    "Dot product of two vectors, producing a single scalar. Supports vectors of any "
    "arity, but only float or double scalar types. Both input vectors must be of the "
    "same scalar type and arity.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_dot_product};

// Arity 2: the two input vectors are treated as (x, y, 0) and the result is the
// z-component of their 3D cross product -- i.e. the signed area of the parallelogram
// they span. The output is a single scalar, not a vector. Fixed at arity 2, so this
// indexes a[0]/a[1] directly instead of looping over a generic arity.
#define DEFINE_VEC_CROSS_AREA_DISPATCH(type, suffix)                             \
  static OrcError _vec_cross_area_##suffix(void *combinations)                   \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_val    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_val == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView a_view = orc_sdk_comb_get_input(combinations, 0);          \
      OrcSdk_DeckView b_view = orc_sdk_comb_get_input(combinations, 1);          \
      type const     *a      = (type const *)orc_sdk_dv_item_ptr(&a_view);       \
      type const     *b      = (type const *)orc_sdk_dv_item_ptr(&b_view);       \
      *out_val               = (type)(a[0] * b[1] - a[1] * b[0]);                \
      combinations           = orc_sdk_comb_advance(combinations);               \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_VEC_CROSS_AREA_DISPATCH(float, f32)
DEFINE_VEC_CROSS_AREA_DISPATCH(double, f64)

// Arity 3: the usual 3D cross product, producing a vec3. Fixed at arity 3, so this
// indexes a[0..2] directly instead of looping over a generic arity.
#define DEFINE_VEC_CROSS_VEC3_DISPATCH(type, suffix)                             \
  static OrcError _vec_cross_vec3_##suffix(void *combinations)                   \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_vec == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView a_view = orc_sdk_comb_get_input(combinations, 0);          \
      OrcSdk_DeckView b_view = orc_sdk_comb_get_input(combinations, 1);          \
      type const     *a      = (type const *)orc_sdk_dv_item_ptr(&a_view);       \
      type const     *b      = (type const *)orc_sdk_dv_item_ptr(&b_view);       \
      out_vec[0]             = (type)(a[1] * b[2] - a[2] * b[1]);                \
      out_vec[1]             = (type)(a[2] * b[0] - a[0] * b[2]);                \
      out_vec[2]             = (type)(a[0] * b[1] - a[1] * b[0]);                \
      combinations           = orc_sdk_comb_advance(combinations);               \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_VEC_CROSS_VEC3_DISPATCH(float, f32)
DEFINE_VEC_CROSS_VEC3_DISPATCH(double, f64)

static OrcError vec_cross_product(uint64_t         ctx,
                                  OrcHandle const *input,
                                  uint64_t         n_inputs,
                                  OrcHandle       *output,
                                  uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 2) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Need exactly two vectors for a cross product.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Cross product only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  // Cross product is only defined between vectors of the exact same type and arity.
  if (input[1].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[1].item_size != first_item_size) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both input vectors must be of the same arity.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity != 2 && arity != 3) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Cross product only supports vectors of arity 2 or 3.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output: a single scalar (area) for arity 2, or a vec3 for arity 3.
  size_t const output_item_size = (arity == 2) ? scalar_size : 3 * scalar_size;
  err = orc_sdk_handle_alloc(first_type_id, output_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 2, so the input pointers/depths can just live on the stack --
  // no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {input + 0, input + 1},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         &output,
                                         (uint8_t const[]) {0},
                                         1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on (arity, type) here, instead of checking either one inside the
  // combinations loop. Each of the four functions below is fully specialized to one
  // fixed arity and one fixed scalar type, with no per-item branching at all.
  if (arity == 2) {
    switch (first_type_id) {
    case ORC_TYPE_F32:
      err = _vec_cross_area_f32(combinations);
      break;
    case ORC_TYPE_F64:
      err = _vec_cross_area_f64(combinations);
      break;
    default:
      ORC_SDK_REQUIRE_WITH_MSG(
        false, "is_float_type guarantees first_type_id is one of the cases above.");
      break;
    }
  }
  else {  // arity == 3
    switch (first_type_id) {
    case ORC_TYPE_F32:
      err = _vec_cross_vec3_f32(combinations);
      break;
    case ORC_TYPE_F64:
      err = _vec_cross_vec3_f64(combinations);
      break;
    default:
      ORC_SDK_REQUIRE_WITH_MSG(
        false, "is_float_type guarantees first_type_id is one of the cases above.");
      break;
    }
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_CROSS_PRODUCT_INFO = {
  .name = "vec_cross_product",
  .desc =
    "Cross product of two vectors, of arity 2 or 3, and either float or double scalar "
    "type. For arity 3, produces the usual vec3 cross product. For arity 2, produces a "
    "single scalar: the signed area of the parallelogram spanned by the two vectors.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_cross_product};

// One type-specific implementation per supported scalar type -- only float and double.
// Each owns the whole combinations loop, with pointers already cast to `type`, so
// vec_length_sq only needs to pick one of these once instead of switching on the type
// for every item. The output here is a single scalar (arity 1), not a vector of the
// same arity as the input -- it's a reduction, not an elementwise op.
#define DEFINE_VEC_LENGTH_SQ_DISPATCH(type, suffix)                               \
  static OrcError _vec_length_sq_##suffix(void *combinations, size_t const arity) \
  {                                                                               \
    while (combinations) {                                                        \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);   \
      type              *out_val    = (type *)orc_sdk_dw_push_empty(out_writer);  \
      if (out_val == NULL) {                                                      \
        orc_sdk_comb_free(combinations);                                          \
        return ORC_ERROR_ALLOC_FAILED;                                            \
      }                                                                           \
      OrcSdk_DeckView v_view = orc_sdk_comb_get_input(combinations, 0);           \
      type const     *v      = (type const *)orc_sdk_dv_item_ptr(&v_view);        \
      type            sum    = 0;                                                 \
      for (size_t j = 0; j < arity; ++j) {                                        \
        sum = (type)(sum + v[j] * v[j]);                                          \
      }                                                                           \
      *out_val     = sum;                                                         \
      combinations = orc_sdk_comb_advance(combinations);                          \
    }                                                                             \
    return ORC_ERROR_NONE;                                                        \
  }

DEFINE_VEC_LENGTH_SQ_DISPATCH(float, f32)
DEFINE_VEC_LENGTH_SQ_DISPATCH(double, f64)

// Same as above, but takes the square root -- the usual vector length/magnitude.
// sqrt_fn is passed in as sqrtf or sqrt so each instantiation calls the correctly
// typed one, matching the (type) casts everywhere else in this family.
#define DEFINE_VEC_LENGTH_DISPATCH(type, suffix, sqrt_fn)                        \
  static OrcError _vec_length_##suffix(void *combinations, size_t const arity)   \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_val    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_val == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView v_view = orc_sdk_comb_get_input(combinations, 0);          \
      type const     *v      = (type const *)orc_sdk_dv_item_ptr(&v_view);       \
      type            sum    = 0;                                                \
      for (size_t j = 0; j < arity; ++j) {                                       \
        sum = (type)(sum + v[j] * v[j]);                                         \
      }                                                                          \
      *out_val     = sqrt_fn(sum);                                               \
      combinations = orc_sdk_comb_advance(combinations);                         \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_VEC_LENGTH_DISPATCH(float, f32, sqrtf)
DEFINE_VEC_LENGTH_DISPATCH(double, f64, sqrt)

static OrcError vec_length_sq(uint64_t         ctx,
                              OrcHandle const *input,
                              uint64_t         n_inputs,
                              OrcHandle       *output,
                              uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need exactly one vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "vec_length_sq only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need a vector of arity 2 or more.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a single scalar, not a vector.
  err = orc_sdk_handle_alloc(first_type_id, scalar_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 1, so we can pass the address of the `input` parameter
  // directly -- no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {0}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_length_sq_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_length_sq_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_LENGTH_SQ_INFO = {
  .name = "vec_length_sq",
  .desc =
    "Squared length (magnitude) of a vector, of arity 2 or more, and either float or "
    "double scalar type. Cheaper than vec_length when you don't need the actual length "
    "(e.g. comparing distances).",
  .n_inputs    = 1,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_length_sq};

static OrcError vec_length(uint64_t         ctx,
                           OrcHandle const *input,
                           uint64_t         n_inputs,
                           OrcHandle       *output,
                           uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need exactly one vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "vec_length only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need a vector of arity 2 or more.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a single scalar, not a vector.
  err = orc_sdk_handle_alloc(first_type_id, scalar_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 1, so we can pass the address of the `input` parameter
  // directly -- no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {0}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_length_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_length_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_LENGTH_INFO = {
  .name = "vec_length",
  .desc =
    "Length (magnitude) of a vector, of arity 2 or more, and either float or double "
    "scalar type.",
  .n_inputs    = 1,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_length};

// One type-specific implementation per supported scalar type -- only float and double.
// Unlike vec_length/vec_length_sq, the output here is a vector of the same arity as
// the input, not a scalar: this is an elementwise scale, not a reduction.
#define DEFINE_VEC_NORMALIZE_DISPATCH(type, suffix, sqrt_fn)                       \
  static OrcError _vec_normalize_##suffix(void *combinations, size_t const arity)  \
  {                                                                                \
    while (combinations) {                                                         \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);    \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer);   \
      if (out_vec == NULL) {                                                       \
        orc_sdk_comb_free(combinations);                                           \
        return ORC_ERROR_ALLOC_FAILED;                                             \
      }                                                                            \
      OrcSdk_DeckView v_view = orc_sdk_comb_get_input(combinations, 0);            \
      type const     *v      = (type const *)orc_sdk_dv_item_ptr(&v_view);         \
      type            sum    = 0;                                                  \
      for (size_t j = 0; j < arity; ++j) {                                         \
        sum = (type)(sum + v[j] * v[j]);                                           \
      }                                                                            \
      type const length = sqrt_fn(sum);                                            \
      /* A zero-length input normalizes to inf/nan components -- standard IEEE-754 \
         division-by-zero behavior for floating point, not treated as an error     \
         here. */                                                                  \
      for (size_t j = 0; j < arity; ++j) {                                         \
        out_vec[j] = (type)(v[j] / length);                                        \
      }                                                                            \
      combinations = orc_sdk_comb_advance(combinations);                           \
    }                                                                              \
    return ORC_ERROR_NONE;                                                         \
  }

DEFINE_VEC_NORMALIZE_DISPATCH(float, f32, sqrtf)
DEFINE_VEC_NORMALIZE_DISPATCH(double, f64, sqrt)

static OrcError vec_normalize(uint64_t         ctx,
                              OrcHandle const *input,
                              uint64_t         n_inputs,
                              OrcHandle       *output,
                              uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need exactly one vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "vec_normalize only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need a vector of arity 2 or more.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a vector of the same arity as the input, unlike
  // vec_length/vec_length_sq which reduce to a single scalar.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 1, so we can pass the address of the `input` parameter
  // directly -- no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {0}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_normalize_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_normalize_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_NORMALIZE_INFO = {
  .name = "vec_normalize",
  .desc =
    "Normalize a vector (scale it to unit length), of arity 2 or more, and either "
    "float or double scalar type. The output vector has the same arity as the input.",
  .n_inputs    = 1,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_normalize};

// One type-specific implementation per supported scalar type -- only float and double.
// Elementwise negate: the output here is a vector of the same arity as the input, not
// a scalar reduction.
#define DEFINE_VEC_NEGATIVE_DISPATCH(type, suffix)                               \
  static OrcError _vec_negative_##suffix(void *combinations, size_t const arity) \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_vec == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView v_view = orc_sdk_comb_get_input(combinations, 0);          \
      type const     *v      = (type const *)orc_sdk_dv_item_ptr(&v_view);       \
      for (size_t j = 0; j < arity; ++j) {                                       \
        out_vec[j] = (type)(-v[j]);                                              \
      }                                                                          \
      combinations = orc_sdk_comb_advance(combinations);                         \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_VEC_NEGATIVE_DISPATCH(float, f32)
DEFINE_VEC_NEGATIVE_DISPATCH(double, f64)

static OrcError vec_negative(uint64_t         ctx,
                             OrcHandle const *input,
                             uint64_t         n_inputs,
                             OrcHandle       *output,
                             uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need exactly one vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "vec_negative only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need a vector of arity 2 or more.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a vector of the same arity as the input.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 1, so we can pass the address of the `input` parameter
  // directly -- no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {0}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_negative_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_negative_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_NEGATIVE_INFO = {
  .name = "vec_negative",
  .desc =
    "Negate a vector (flip the sign of every component), of arity 2 or more, and "
    "either float or double scalar type. The output vector has the same arity as the "
    "input.",
  .n_inputs    = 1,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_negative};

// One type-specific implementation per supported scalar type -- only float and double.
// Scales the input vector so its length matches a separately provided target length.
// The output here is a vector of the same arity as the input, not a scalar.
#define DEFINE_VEC_SCALE_TO_LENGTH_DISPATCH(type, suffix, sqrt_fn)                      \
  static OrcError _vec_scale_to_length_##suffix(void *combinations, size_t const arity) \
  {                                                                                     \
    while (combinations) {                                                              \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);         \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer);        \
      if (out_vec == NULL) {                                                            \
        orc_sdk_comb_free(combinations);                                                \
        return ORC_ERROR_ALLOC_FAILED;                                                  \
      }                                                                                 \
      OrcSdk_DeckView v_view     = orc_sdk_comb_get_input(combinations, 0);             \
      OrcSdk_DeckView len_view   = orc_sdk_comb_get_input(combinations, 1);             \
      type const     *v          = (type const *)orc_sdk_dv_item_ptr(&v_view);          \
      type const     *target_len = (type const *)orc_sdk_dv_item_ptr(&len_view);        \
      type            sum        = 0;                                                   \
      for (size_t j = 0; j < arity; ++j) {                                              \
        sum = (type)(sum + v[j] * v[j]);                                                \
      }                                                                                 \
      type const current_len = sqrt_fn(sum);                                            \
      type const scale       = (type)(*target_len / current_len);                       \
      /* A zero-length input scales to inf/nan components -- standard IEEE-754          \
         division-by-zero behavior for floating point, not treated as an error          \
         here. */                                                                       \
      for (size_t j = 0; j < arity; ++j) {                                              \
        out_vec[j] = (type)(v[j] * scale);                                              \
      }                                                                                 \
      combinations = orc_sdk_comb_advance(combinations);                                \
    }                                                                                   \
    return ORC_ERROR_NONE;                                                              \
  }

DEFINE_VEC_SCALE_TO_LENGTH_DISPATCH(float, f32, sqrtf)
DEFINE_VEC_SCALE_TO_LENGTH_DISPATCH(double, f64, sqrt)

static OrcError vec_scale_to_length(uint64_t         ctx,
                                    OrcHandle const *input,
                                    uint64_t         n_inputs,
                                    OrcHandle       *output,
                                    uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 2) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Need exactly two inputs: a vector and a target length.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(ctx,
                           ORC_MSG_LEVEL_ERROR,
                           "vec_scale_to_length only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (arity < 2) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need a vector of arity 2 or more.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // The target length must be a single scalar of the same type as the vector.
  if (input[1].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx,
      ORC_MSG_LEVEL_ERROR,
      "The target length must be of the same scalar type as the vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[1].item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  if (input[1].item_size != scalar_size) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "The target length must be a single scalar value.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a vector of the same arity as the input.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 2, so the input pointers/depths can just live on the stack --
  // no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {input + 0, input + 1},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         &output,
                                         (uint8_t const[]) {0},
                                         1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _vec_scale_to_length_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _vec_scale_to_length_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const VEC_SCALE_TO_LENGTH_INFO = {
  .name = "vec_scale_to_length",
  .desc =
    "Scale a vector so its length matches a given target length, of arity 2 or "
    "more, and either float or double scalar type. The target length must be a "
    "single scalar of the same scalar type. The output vector has the same arity as "
    "the input.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_scale_to_length};

static OrcError vec_components(uint64_t         ctx,
                               OrcHandle const *input,
                               uint64_t         n_inputs,
                               OrcHandle       *output,
                               uint64_t         n_outputs)
{
  // Validate input/output counts.
  if (n_inputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need exactly one vector.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs < 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Need at least 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_primitive_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Vector components must be primitive scalar types");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  if (n_outputs > arity) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Cannot request more outputs than the vector's arity.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate one scalar output per requested component.
  for (uint64_t k = 0; k < n_outputs; ++k) {
    err = orc_sdk_handle_alloc(first_type_id, scalar_size, output + k);
    if (err)
      return err;
  }
  // n_inputs is fixed at 1, so the input pointer/depth can just live on the stack --
  // only the output side needs a heap-growing array, since n_outputs is variadic.
  void             *combinations  = NULL;
  OrcHandle       **output_ptrs   = NULL;
  uint8_t          *output_depths = NULL;
  // Above three need to be cleaned up in all exit paths.
  {
    // Depths array -- everything at depth 0.
    uint8_t const zero_depth = 0;
    orc_sdk_arr_resize(output_depths, n_outputs);
    orc_sdk_arr_fill(output_depths, zero_depth);
    // Pack output handle pointers into an array.
    orc_sdk_arr_reserve(output_ptrs, n_outputs);
    for (uint64_t k = 0; k < n_outputs; ++k) {
      err = orc_sdk_arr_push(output_ptrs, output + k);
      if (err)
        goto cleanup;
    }
    combinations = orc_sdk_comb_init(
      &input, (uint8_t const[]) {0}, 1, output_ptrs, output_depths, n_outputs);
  }
  if (combinations == NULL) {
    err = ORC_ERROR_INVALID_COMBINATIONS;
    goto cleanup;
  }
  // No type dispatch needed here -- this just copies raw component bytes around, it
  // doesn't do any arithmetic, so there's nothing for a per-type specialization to buy
  // us (same reasoning as make_vec's use of memcpy).
  {
    OrcError status = ORC_ERROR_NONE;
    while (combinations) {
      OrcSdk_DeckView in_view = orc_sdk_comb_get_input(combinations, 0);
      char const     *v       = (char const *)orc_sdk_dv_item_ptr(&in_view);
      for (uint64_t k = 0; k < n_outputs; ++k) {
        OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, k);
        char              *out_ptr    = (char *)orc_sdk_dw_push_empty(out_writer);
        if (out_ptr == NULL) {
          status = ORC_ERROR_ALLOC_FAILED;
          break;
        }
        memcpy(out_ptr, v + k * scalar_size, scalar_size);
      }
      if (status != ORC_ERROR_NONE) {
        break;
      }
      combinations = orc_sdk_comb_advance(combinations);
    }
    // Whether the loop above exhausted `combinations` naturally (which frees it
    // internally, see orc_sdk_comb_advance) or broke out early on error, this call
    // is always needed: it's a safe no-op in the first case, and does the actual
    // freeing in the second.
    orc_sdk_comb_free(combinations);
    combinations = NULL;
    err          = status;
  }
  if (err == ORC_ERROR_NONE) {
    for (uint64_t k = 0; k < n_outputs; ++k) {
      orc_sdk_oh_update(output + k);
    }
  }
cleanup:
  orc_sdk_comb_free(combinations);
  orc_sdk_arr_free(output_ptrs);
  orc_sdk_arr_free(output_depths);
  return err;
}

OrcFuncInfo const VEC_COMPONENTS_INFO = {
  .name = "vec_components",
  .desc =
    "Extract the components of a vector -- the opposite of make_vec. One vector "
    "input, and one or more scalar outputs; output k receives the vector's k-th "
    "component. The number of outputs may be less than the vector's arity (the "
    "remaining components are simply not output), but not more. Supports any "
    "primitive scalar type.",
  .n_inputs    = 1,
  .n_outputs   = ORC_ARGS_VARIADIC,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_components};

// One type-specific implementation per supported scalar type -- only float and double.
// Elementwise linear interpolation: a + t * (b - a). The output here is a vector of
// the same arity as a and b, not a scalar.
#define DEFINE_LERP_DISPATCH(type, suffix)                                       \
  static OrcError _lerp_##suffix(void *combinations, size_t const arity)         \
  {                                                                              \
    while (combinations) {                                                       \
      OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);  \
      type              *out_vec    = (type *)orc_sdk_dw_push_empty(out_writer); \
      if (out_vec == NULL) {                                                     \
        orc_sdk_comb_free(combinations);                                         \
        return ORC_ERROR_ALLOC_FAILED;                                           \
      }                                                                          \
      OrcSdk_DeckView a_view = orc_sdk_comb_get_input(combinations, 0);          \
      OrcSdk_DeckView b_view = orc_sdk_comb_get_input(combinations, 1);          \
      OrcSdk_DeckView t_view = orc_sdk_comb_get_input(combinations, 2);          \
      type const     *a      = (type const *)orc_sdk_dv_item_ptr(&a_view);       \
      type const     *b      = (type const *)orc_sdk_dv_item_ptr(&b_view);       \
      type const     *t      = (type const *)orc_sdk_dv_item_ptr(&t_view);       \
      for (size_t j = 0; j < arity; ++j) {                                       \
        out_vec[j] = (type)(a[j] + *t * (b[j] - a[j]));                          \
      }                                                                          \
      combinations = orc_sdk_comb_advance(combinations);                         \
    }                                                                            \
    return ORC_ERROR_NONE;                                                       \
  }

DEFINE_LERP_DISPATCH(float, f32)
DEFINE_LERP_DISPATCH(double, f64)

static OrcError lerp(uint64_t         ctx,
                     OrcHandle const *input,
                     uint64_t         n_inputs,
                     OrcHandle       *output,
                     uint64_t         n_outputs)
{
  // Validate input counts.
  if (n_inputs != 3) {
    orc_sdk_report_message(
      ctx,
      ORC_MSG_LEVEL_ERROR,
      "Need exactly three inputs: two vectors and an interpolation parameter.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  else if (n_outputs != 1) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Expected 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcTypeId const first_type_id = input[0].type_id;
  if (!is_float_type(first_type_id)) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "lerp only supports float or double vectors.");
    return ORC_ERROR_TYPE_MISMATCH;
  }
  uint64_t const first_item_size = input[0].item_size;
  if (first_item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  // a and b must be of the same type and arity. Unlike vec_length/vec_normalize/
  // vec_negative/vec_scale_to_length, arity 1 (a plain scalar) is allowed here.
  if (input[1].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both vectors must be of the same type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[1].item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  if (input[1].item_size != first_item_size) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Both vectors must be of the same arity.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcSdk_TypeInfo type_info = {0};
  OrcError        err       = orc_sdk_get_type_info(first_type_id, &type_info);
  if (err)
    return err;
  size_t const scalar_size = type_info.item_size;
  if (scalar_size == 0 || (first_item_size % scalar_size) != 0) {
    // The size of the aggregate type must be a multiple of the scalar size.
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  size_t const arity = first_item_size / scalar_size;
  // The interpolation parameter must be a single scalar of the same type.
  if (input[2].type_id != first_type_id) {
    orc_sdk_report_message(
      ctx,
      ORC_MSG_LEVEL_ERROR,
      "The interpolation parameter must be of the same scalar type as the vectors.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input[2].item_size == 0) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid handle.");
    return ORC_ERROR_INVALID_HANDLE;
  }
  if (input[2].item_size != scalar_size) {
    orc_sdk_report_message(ctx,
                           ORC_MSG_LEVEL_ERROR,
                           "The interpolation parameter must be a single scalar value.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  // Allocate output -- a vector of the same arity as a and b.
  err = orc_sdk_handle_alloc(first_type_id, first_item_size, output);
  if (err)
    return err;
  // n_inputs is fixed at 3, so the input pointers/depths can just live on the stack --
  // no heap allocation needed for a size known at compile time.
  ORC_SDK_REQUIRE_WITH_MSG(
    n_outputs == 1,
    "We already checked before. This is just to make sure we don't go out of sync.");
  void *combinations =
    orc_sdk_comb_init((OrcHandle const *[]) {input + 0, input + 1, input + 2},
                      (uint8_t const[]) {0, 0, 0},
                      3,
                      &output,
                      (uint8_t const[]) {0},
                      1);
  if (combinations == NULL) {
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  // Dispatch once on the component type here, instead of checking it inside the
  // combinations loop.
  switch (first_type_id) {
  case ORC_TYPE_F32:
    err = _lerp_f32(combinations, arity);
    break;
  case ORC_TYPE_F64:
    err = _lerp_f64(combinations, arity);
    break;
  default:
    ORC_SDK_REQUIRE_WITH_MSG(
      false, "is_float_type guarantees first_type_id is one of the cases above.");
    break;
  }
  // The dispatch functions above always fully consume `combinations`: either they
  // exhaust it (which frees it internally, see orc_sdk_comb_advance), or they free it
  // explicitly on error.
  if (err == ORC_ERROR_NONE) {
    orc_sdk_oh_update(output);
  }
  return err;
}

OrcFuncInfo const LERP_INFO = {
  .name = "lerp",
  .desc =
    "Linearly interpolate between two vectors a and b by parameter t: a + t * (b - "
    "a). Supports vectors (or plain scalars) of arity 1 or more, and either float or "
    "double scalar type. a and b must be of the same scalar type and arity; t must "
    "be a single scalar of the same type.",
  .n_inputs    = 3,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = lerp};
