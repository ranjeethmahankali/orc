#include <orc_sdk/orc_sdk.h>
#include <stdlib.h>

// ==============================
// Functions
// ==============================

extern OrcFuncInfo const FLATTEN_DECK_INFO;
extern OrcFuncInfo const LIST_LENGTH_INFO;

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
  // Register custom types - none at the moment.
  plugin_data_out->types   = NULL;
  plugin_data_out->n_types = 0;
  // Register functions.
  static OrcFuncInfo *FUNCTIONS = NULL;
  orc_sdk_arr_push(FUNCTIONS, FLATTEN_DECK_INFO);
  orc_sdk_arr_push(FUNCTIONS, LIST_LENGTH_INFO);
  plugin_data_out->functions   = FUNCTIONS;
  plugin_data_out->n_functions = orc_sdk_arr_len(FUNCTIONS);
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

OrcError orc_deck_serialize(uint64_t const      ctx,
                            OrcHandle const    *handle,
                            OrcSerializeWriteFn write_fn)
{
  OrcError err = orc_sdk_serialize_handle_header(ctx, handle, write_fn);
  if (err != ORC_ERROR_NONE) {
    return err;
  }
  ORC_SDK_REQUIRE(handle->item_size > 0);
  // IMPORTANT: This only works because this plugin doesn't define custom types with any
  // pointer indirection. All the builtin types are value types.
  err = write_fn(ctx, handle->items, handle->item_size * handle->n_items);
  if (err != ORC_ERROR_NONE) {
    return err;
  }
  return ORC_ERROR_NONE;
}

OrcError orc_deck_deserialize(uint64_t const ctx,
                              void const    *buf,
                              uint64_t const buf_len,
                              OrcHandle     *out)
{
  OrcStrView src   = {.start = (char *)buf, .end = (char *)buf + buf_len};
  OrcMark   *marks = NULL;
  OrcError   err   = orc_sdk_deserialize_handle_header(ctx, &src, out, &marks);
  if (err != ORC_ERROR_NONE) {
    return err;
  }

  (void)src;
  (void)ctx;
  (void)out;
  ORC_SDK_TODO("Not implemented");
}
