#include <orc_abi.h>
#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include <string.h>

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
  size_t          output_arity  = 0;
  size_t         *input_arities = NULL;
  OrcTypeId const first_type_id = input[0].type_id;
  size_t          scalar_size   = 0;
  {  // Make sure all the inputs (components) are the same type. And compute the output
     // arity at the same time.
    for (size_t i = 0; i < n_inputs; ++i) {
      if (input[i].type_id != first_type_id) {
        orc_sdk_report_message(ctx,
                               ORC_MSG_LEVEL_ERROR,
                               "All components of the vector must of of the same type.");
        orc_sdk_arr_free(input_arities);
        return ORC_ERROR_INVALID_ARGUMENTS;
      }
      OrcSdk_TypeInfo type_info = {0};
      OrcError        err       = orc_sdk_get_type_info(input[i].type_id, &type_info);
      if (err != ORC_ERROR_NONE) {
        orc_sdk_arr_free(input_arities);
        return err;
      }
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
        orc_sdk_arr_free(input_arities);
        return ORC_ERROR_INVALID_HANDLE;
      }
      if (input[i].item_size % scalar_size) {
        // The size of the aggregate type must be a multiple of the scalar size.
        orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Invalid aggregate type.");
        orc_sdk_arr_free(input_arities);
        return ORC_ERROR_INVALID_ARGUMENTS;
      }
      size_t const arity = input[0].item_size / scalar_size;
      err                = orc_sdk_arr_push(input_arities, arity);
      output_arity += arity;
    }
  }
  // Allocate output.
  orc_sdk_handle_alloc(first_type_id, scalar_size * output_arity, output);
  // Stride over inputs with list combinations, and assign the output.
  void *combinations = NULL;
  {
    uint8_t *input_depths = NULL;
    orc_sdk_arr_resize(input_depths, n_inputs);
    uint8_t const zero_depth = 0;
    orc_sdk_arr_fill(input_depths, zero_depth);
    ORC_SDK_REQUIRE_WITH_MSG(
      n_outputs == 1,
      "We already checked before. This is just to make sure we don't go out of sync.");
    combinations = orc_sdk_comb_init(
      &input, input_depths, n_inputs, &output, (uint8_t const[]) {0}, 1);
    orc_sdk_arr_free(input_depths);
  }
  while (combinations) {
    OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);
    // Casting to char* so that I can increment by byte.
    char *out_vec = (char *)orc_sdk_dw_push_empty(out_writer);
    for (size_t i = 0; i < n_inputs; ++i) {
      OrcSdk_DeckView in_view    = orc_sdk_comb_get_input(combinations, i);
      void const     *in_vec     = orc_sdk_dv_item_ptr(&in_view);
      size_t const    input_size = input_arities[i] * scalar_size;
      memcpy(out_vec, in_vec, input_size);
      out_vec += input_size;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
  orc_sdk_arr_free(input_arities);
  return ORC_ERROR_NONE;
}

OrcFuncInfo const MAKE_VEC_INFO = {
  .name = "make_vec",
  .desc =
    "Create a vector from it's components. The arity of the vector will be the same as "
    "the number of input components provided. Supports any scalar type.",
  .n_inputs    = ORC_ARGS_VARIADIC,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = make_vec};

static OrcError vec_add(uint64_t         ctx,
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
    "Cross product of two vectors. Supports vectors of any arity, and scalar type, "
    "as "
    "long as the scalar type supports multiplication and addition.",
  .n_inputs    = 2,
  .n_outputs   = 1,
  .input_args  = NULL,
  .output_args = NULL,
  .func        = vec_cross_product};
