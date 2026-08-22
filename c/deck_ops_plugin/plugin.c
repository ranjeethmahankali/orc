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

OrcError orc_deck_serialize(uint64_t const ctx, OrcHandle const *handle)
{
  OrcError err = orc_sdk_serialize_handle_header(ctx, handle);
  if (err != ORC_ERROR_NONE) {
    return err;
  }
  ORC_SDK_REQUIRE(handle->item_size > 0);
  // IMPORTANT: This only works because this plugin doesn't define custom types with any
  // pointer indirection. All the builtin types are value types.
  if (handle->n_items > 0 && handle->items != NULL) {
    err =
      orc_sdk_host_serial_write(ctx, handle->items, handle->item_size * handle->n_items);
  }
  return ORC_ERROR_NONE;
}

OrcError orc_deck_deserialize(uint64_t const ctx,
                              void const    *buf,
                              uint64_t const buf_len,
                              OrcHandle     *out)
{
  // This implementation is very naive and simple. It can affort to be so because this
  // plugin doesn't implement any custom types with pointer indirection.
  OrcStrView src   = {.start = (char *)buf, .end = (char *)buf + buf_len};
  OrcMark   *marks = NULL;
  OrcError   err   = orc_sdk_deserialize_handle_header(ctx, &src, out, &marks);
  if (err != ORC_ERROR_NONE) {
    return err;
  }
  void *deck = NULL;
  {  // We just want to use the registry mechanism of orc_deck_alloc. Otherwise this deck
     // won't be in the registry.
    OrcHandle temp_handle = {0};
    temp_handle.handle    = out->handle;
    err                   = orc_sdk_handle_alloc(out->type_id, &temp_handle);
    if (err != ORC_ERROR_NONE) {
      orc_sdk_arr_free(marks);
      return err;
    }
    if (out->item_size != temp_handle.item_size) {
      orc_sdk_arr_free(marks);
      orc_sdk_handle_free(&temp_handle);
      return ORC_ERROR_SERIALIZATION_ERROR;
    }
    deck = _orc_sdk_deck_grow_capacity(
      (void *)temp_handle.items, temp_handle.item_size, out->n_items);
    ORC_SDK_REQUIRE(deck != NULL);
  }
  _OrcSdk_DeckHeader *header = _orc_sdk_deck_header(deck);
  header->count              = out->n_items;
  header->item_size          = out->item_size;
  header->marks              = marks;
  out->items                 = deck;
  orc_sdk_oh_update(out);
  if (out->n_items > 0) {
    err = orc_sdk_sv_read_bytes(&src, deck, out->item_size * out->n_items);
    if (err != ORC_ERROR_NONE) {
      orc_sdk_handle_free(out);
      return err;
    }
  }
  // We must have fully consumed the sv by this point. Otherwise the deserialization
  // didn't work properly.
  if (!orc_sv_is_empty(src)) {
    orc_sdk_handle_free(out);
    return ORC_ERROR_SERIALIZATION_ERROR;
  }
  orc_sdk_deck_calc_strides(header);
  orc_sdk_oh_update(out);
  return ORC_ERROR_NONE;
}
