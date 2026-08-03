#include "functions.h"

#include <orc_sdk/orc_sdk.h>
#include "orc_abi.h"

OrcFuncInfo const FLATTEN_DECK_INFO = {
  .name = "flatten_deck",
  .desc = "Flattens the input deck into one plain list.",
  .func = flatten_deck};

void flatten_deck(uint64_t         ctx,
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
