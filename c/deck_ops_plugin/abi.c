#include <orc_sdk/orc_sdk.h>

#include "functions.h"

// ==============================
// Plugin functions (forward declarations)
// ==============================

static OrcFuncInfo const FUNCTIONS[] = {
  {.name = "flatten_deck",
   .desc = "Flattens the input deck into one plain list.",
   .func = flatten_deck}};

// ==============================
// Required ABI exports
// ==============================

OrcError orc_plugin_init(OrcHost const *host, OrcPlugin *plugin_data_out)
{ /* This plugin doesn't define any custom types. So we don't have to register
     item_size_fn, copy_fn, free_fn etc. with the SDK.*/
  if (host->abi_version != ORC_ABI_VERSION) {
    return ORC_ERROR_ABI_VERSION_MISMATCH;
  }
  orc_sdk_init(host, NULL);
  plugin_data_out->abi_version = ORC_ABI_VERSION;
  plugin_data_out->name        = "deck_ops";
  plugin_data_out->desc        = "Deck/Container operations.";
  plugin_data_out->types       = NULL;
  plugin_data_out->n_types     = 0;
  plugin_data_out->functions   = FUNCTIONS;
  plugin_data_out->n_functions = sizeof(FUNCTIONS) / sizeof(FUNCTIONS[0]);
  return ORC_ERROR_NONE;
}

OrcError orc_deck_alloc(OrcTypeId const id, OrcHandle *const out)
{
  return orc_sdk_handle_alloc(id, out);
}

OrcError orc_deck_free(OrcHandle *const handle)
{
  return orc_sdk_handle_free(handle);
}

OrcError orc_deck_from_proxy(OrcHandle const   *inputs,
                             uint64_t const     n_inputs,
                             OrcProxyType const proxy_type,
                             OrcHandle const   *proxy,
                             OrcHandle         *out)
{
  return orc_sdk_deck_from_proxy(inputs, n_inputs, proxy_type, proxy, out);
}
