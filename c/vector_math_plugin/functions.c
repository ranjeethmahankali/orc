#include <orc_abi.h>
#include <orc_sdk/orc_sdk.h>
#include <stdint.h>

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
  {  // Make sure all the inputs (components) are the same type. And they cannot be
     // aggregate types.
    for (size_t i = 1; i < n_inputs; ++i) {}
  }
  (void)input;
  (void)output;
  ORC_SDK_TODO("Not implemented...");
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
