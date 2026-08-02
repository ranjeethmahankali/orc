#include <orc_abi.h>
#include <orc_sdk/orc_sdk.h>

// ==============================
// Internal state
// ==============================

static OrcHost const *s_host = NULL;

// ==============================
// Plugin functions (forward declarations)
// ==============================

// TODO: implement and add to s_functions below.
// static void deck_ops_flatten(uint64_t ctx, OrcHandle const *inputs, uint64_t n_inputs,
//                              OrcHandle *outputs, uint64_t n_outputs);

static OrcFuncInfo const s_functions[] = {{.name = NULL, .desc = NULL, .func = NULL}};

// ==============================
// Required ABI exports
// ==============================

ORC_PLUGIN_EXPORT OrcError orc_plugin_init(OrcHost const *host,
                                           OrcPlugin     *plugin_data_out)
{
  if (host->abi_version != ORC_ABI_VERSION) {
    return ORC_ERROR_ABI_VERSION_MISMATCH;
  }
  if (s_host != NULL) {
    return ORC_ERROR_PLUGIN_ALREADY_INITIALIZED;
  }
  s_host                       = host;
  plugin_data_out->abi_version = ORC_ABI_VERSION;
  plugin_data_out->name        = "deck_ops";
  plugin_data_out->desc        = "Deck operations: flatten, graft, simplify, etc.";
  plugin_data_out->types       = NULL;
  plugin_data_out->n_types     = 0;
  plugin_data_out->functions   = s_functions;
  plugin_data_out->n_functions = sizeof(s_functions) / sizeof(s_functions[0]);
  return ORC_ERROR_NONE;
}

ORC_PLUGIN_EXPORT OrcError orc_deck_alloc(OrcTypeId const id, OrcHandle *const out)
{
  // This plugin defines no custom types.
  (void)id;
  (void)out;
  return ORC_ERROR_TYPE_MISMATCH;
}

ORC_PLUGIN_EXPORT OrcError orc_deck_free(OrcHandle *const handle)
{
  // This plugin defines no custom types.
  (void)handle;
  return ORC_ERROR_TYPE_MISMATCH;
}

ORC_PLUGIN_EXPORT OrcError orc_deck_from_proxy(OrcHandle const   *inputs,
                                               uint64_t const     n_inputs,
                                               OrcProxyType const proxy_type,
                                               OrcHandle const   *proxy,
                                               OrcHandle         *out)
{
  // This plugin defines no custom types.
  (void)inputs;
  (void)n_inputs;
  (void)proxy_type;
  (void)proxy;
  (void)out;
  return ORC_ERROR_TYPE_MISMATCH;
}
