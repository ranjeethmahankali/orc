#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include <string.h>
#include "orc_abi.h"

static bool is_type_known(OrcTypeId const type_id, size_t *item_size_out)
{
  bool is_known  = false;
  *item_size_out = 0;
  switch (type_id) {
    // Unsigned integers.
  case ORC_TYPE_U8:
    *item_size_out = sizeof(uint8_t);
    is_known       = true;
    break;
  case ORC_TYPE_U16:
    *item_size_out = sizeof(uint16_t);
    is_known       = true;
    break;
  case ORC_TYPE_U32:
    *item_size_out = sizeof(uint32_t);
    is_known       = true;
    break;
  case ORC_TYPE_U64:
    *item_size_out = sizeof(uint64_t);
    is_known       = true;
    break;
    // Scalars.
  case ORC_TYPE_F32:
    *item_size_out = sizeof(float);
    is_known       = true;
    break;
  case ORC_TYPE_F64:
    *item_size_out = sizeof(double);
    is_known       = true;
    break;
    // Signed integers.
  case ORC_TYPE_I8:
    *item_size_out = sizeof(int8_t);
    is_known       = true;
    break;
  case ORC_TYPE_I16:
    *item_size_out = sizeof(int16_t);
    is_known       = true;
    break;
  case ORC_TYPE_I32:
    *item_size_out = sizeof(int32_t);
    is_known       = true;
    break;
  case ORC_TYPE_I64:
    *item_size_out = sizeof(int64_t);
    is_known       = true;
    break;
  case ORC_TYPE_PROXY:
    *item_size_out = sizeof(OrcItemProxy);
    is_known       = true;
    break;
  default:
    break;
  }
  return is_known;
}

static OrcError list_length(uint64_t         ctx,
                            OrcHandle const *input,
                            uint64_t         n_inputs,
                            OrcHandle       *output,
                            uint64_t         n_outputs)
{
  // Validate inputs.
  if (n_inputs != 1 || n_outputs != 1) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "This function expects 1 input, and produces 1 output.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (input == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Input handle is a null pointer.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (output == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Output handle is a null pointer.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  {  // Allocate outputs.
    OrcError const err = orc_sdk_handle_alloc(ORC_TYPE_U64, output);
    if (err != ORC_ERROR_NONE) {
      orc_sdk_report_message(
        ctx, ORC_MSG_LEVEL_ERROR, "Unable to allocate the output deck.");
      return err;
    }
  }  // Initialize list processing.
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {1}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Unable to iterate over the input data.");
    return ORC_ERROR_INVALID_COMBINATIONS;
  }
  while (combinations) {
    OrcSdk_DeckView    in_list    = orc_sdk_comb_get_input(combinations, 0);
    OrcSdk_DeckWriter *out_writer = orc_sdk_comb_get_output(combinations, 0);
    ORC_SDK_REQUIRE_WITH_MSG(
      in_list.depth == 1 && out_writer->depth == 0,
      "The combinations stuff is not working right. This should never happen.");
    uint64_t *out_ptr = (uint64_t *)orc_sdk_dw_push_empty(out_writer);
    *out_ptr          = (uint64_t)orc_sdk_dv_len(&in_list);
    // Go to the next list processing iteration.
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
  orc_sdk_oh_update(output);
  return ORC_ERROR_NONE;
}

OrcFuncInfo const LIST_LENGTH_INFO = {.name        = "list_length",
                                      .desc        = "Outputs the length of a list.",
                                      .n_inputs    = 1,
                                      .n_outputs   = 1,
                                      .input_args  = NULL,  // Any type is allowed.
                                      .output_args = NULL,  // Any type is allowed.
                                      .func        = list_length};

static OrcError flatten_deck(uint64_t         ctx,
                             OrcHandle const *inputs,
                             uint64_t         n_inputs,
                             OrcHandle       *outputs,
                             uint64_t         n_outputs)
{
  // Validate inputs.
  if (n_inputs != n_outputs) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "The number of inputs and outputs must be the same.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (inputs == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Input handle is a null pointer.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  if (outputs == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Output handle is a null pointer.");
    return ORC_ERROR_INVALID_ARGUMENTS;
  }
  OrcHandle const *input  = inputs;
  OrcHandle       *output = outputs;
  for (size_t i = 0; i < n_inputs; ++i) {
    // Create a proxy on the stack - does not need a free_fn.
    OrcHandle      proxy        = {0};
    OrcMark        first_mark   = {0};
    uint64_t const first_stride = input->n_items;
    uint64_t const zero         = 0;
    proxy.marks                 = &first_mark;
    proxy.stride_offset         = &zero;
    proxy.n_marks               = 1;
    proxy.strides               = &first_stride;
    proxy.type_id               = ORC_TYPE_PROXY;
    memcpy(proxy.dims, input->dims, sizeof(OrcDims));
    // If we own the type, then we don't need to defer to the host.
    size_t item_size = 0;
    if (is_type_known(input->type_id, &item_size)) {
      ORC_SDK_REQUIRE_WITH_MSG(
        item_size != 0,
        "If this plugin recognized this type, the size should never be zero.");
      OrcError const err =
        orc_sdk_deck_from_proxy(input, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, output);
      if (err != ORC_ERROR_NONE) {
        orc_sdk_report_message(
          ctx, ORC_MSG_LEVEL_ERROR, "Unable to create a flattened deck.");
        return err;
      }
    }
    else {
      OrcError const err = orc_sdk_host_create_proxy_deck(
        input, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, output);
      if (err != ORC_ERROR_NONE) {
        orc_sdk_report_message(
          ctx, ORC_MSG_LEVEL_ERROR, "Unable to create a flattened deck.");
        return err;
      }
    }
    ++input;
    ++output;
  }
  return ORC_ERROR_NONE;
}

OrcFuncInfo const FLATTEN_DECK_INFO = {
  .name        = "flatten_deck",
  .desc        = "Flattens the input deck into one plain list.",
  .n_inputs    = ORC_ARGS_VARIADIC,
  .n_outputs   = ORC_ARGS_VARIADIC,
  .input_args  = NULL,  // Any type is allowed.
  .output_args = NULL,  // Any type is allowed.
  .func        = flatten_deck};
