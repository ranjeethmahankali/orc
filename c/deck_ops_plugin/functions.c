#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include "orc_abi.h"

static void flatten_deck(uint64_t         ctx,
                         OrcHandle const *inputs,
                         uint64_t         n_inputs,
                         OrcHandle       *outputs,
                         uint64_t         n_outputs)
{
  (void)ctx;
  (void)inputs;
  (void)n_inputs;
  (void)outputs;
  (void)n_outputs;
  ORC_SDK_TODO("Not implemented");
}

OrcFuncInfo const FLATTEN_DECK_INFO = {
  .name = "flatten_deck",
  .desc = "Flattens the input deck into one plain list.",
  .func = flatten_deck};

static void list_length(uint64_t         ctx,
                        OrcHandle const *input,
                        uint64_t         n_inputs,
                        OrcHandle       *output,
                        uint64_t         n_outputs)
{
  // Validate inputs.
  if (n_inputs != 1 || n_outputs != 1) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "This function expects 1 input, and produces 1 output.");
    return;
  }
  if (input == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Input handle is a null pointer.");
    return;
  }
  if (output == NULL) {
    orc_sdk_report_message(ctx, ORC_MSG_LEVEL_ERROR, "Output handle is a null pointer.");
    return;
  }
  // Allocate outputs.
  orc_sdk_oh_ensure_alloc(ORC_TYPE_U64, output);
  // Initialize list processing.
  void *combinations = orc_sdk_comb_init(
    &input, (uint8_t const[]) {1}, 1, &output, (uint8_t const[]) {0}, 1);
  if (combinations == NULL) {
    orc_sdk_report_message(
      ctx, ORC_MSG_LEVEL_ERROR, "Unable to iterate over the input data.");
    return;
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
}

OrcFuncInfo const LIST_LENGTH_INFO = {.name = "list_length",
                                      .desc = "Outputs the length of a list.",
                                      .func = list_length};
