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
    "Add two vectors component wise. Works for vectors of any arities, and works for any "
    "scalar type that supports addition. All the input vectors must be of the same "
    "scalar type and arity. E.g. I cannot add dvec3 with vec2.",
  .n_inputs    = ORC_ARGS_VARIADIC,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_add};

static OrcError vec_subtract(uint64_t         ctx,
                             OrcHandle const *input,
                             uint64_t         n_inputs,
                             OrcHandle       *output,
                             uint64_t         n_outputs)
{
  (void)ctx;
  (void)input;
  (void)n_inputs;
  (void)output;
  (void)n_outputs;

  ORC_SDK_TODO("Not implemented...");
}

OrcFuncInfo const VEC_SUBTRACT_INFO = {
  .name = "vec_subtract",
  .desc =
    "Subtract the second vector from the first. Supports vectors of any arity, or scalar "
    "type that supports subtraction.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_subtract};

static OrcError vec_dot_product(uint64_t         ctx,
                                OrcHandle const *input,
                                uint64_t         n_inputs,
                                OrcHandle       *output,
                                uint64_t         n_outputs)
{
  (void)ctx;
  (void)input;
  (void)n_inputs;
  (void)output;
  (void)n_outputs;

  ORC_SDK_TODO("Not implemented...");
}

OrcFuncInfo const VEC_DOT_PRODUCT_INFO = {
  .name = "vec_dot_product",
  .desc =
    "Dot product of two vectors. Supports vectors of any arity, and scalar type, as long "
    "as the scalar type supports multiplication and addition.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_dot_product};

static OrcError vec_cross_product(uint64_t         ctx,
                                  OrcHandle const *input,
                                  uint64_t         n_inputs,
                                  OrcHandle       *output,
                                  uint64_t         n_outputs)
{
  (void)ctx;
  (void)input;
  (void)n_inputs;
  (void)output;
  (void)n_outputs;

  ORC_SDK_TODO("Not implemented...");
}

OrcFuncInfo const VEC_CROSS_PRODUCT_INFO = {
  .name = "vec_cross_product",
  .desc =
    "Cross product of two vectors. Supports vectors of floating point scalar types, of "
    "arity 3.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_cross_product};
