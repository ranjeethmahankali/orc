mod deck;
pub use deck::{Combinations, Deck, DeckView, DeckWriter};

mod ffi;
pub use ffi::{ProxyType, TOrcData, TOrcPluginAdaptor, dims_divide, dims_multiply, dims_pow};

mod bindings;
pub use bindings::{
    OrcFuncInfo, OrcHandle, OrcHost, OrcHostCallbackAPI, OrcHostMemoryAPI, OrcItemProxy, OrcMark,
    OrcPlugin, OrcTypeId, OrcTypeInfo,
};
// Export all the type tags and other constants.
pub use bindings::{
    ORC_DECK_PROXY_COPY_ALL, ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, ORC_DIM_CURRENT,
    ORC_DIM_LENGTH, ORC_DIM_LUMINOSITY, ORC_DIM_MASS, ORC_DIM_SUBSTANCE, ORC_DIM_TEMPERATURE,
    ORC_DIM_TIME, ORC_ERROR_INVALID_DIMENSIONS, ORC_ERROR_INVALID_HANDLE, ORC_ERROR_NONE,
    ORC_ERROR_TYPE_MISMATCH, ORC_ERROR_UNKNOWN, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32,
    ORC_I64, ORC_MSG_LEVEL_DEBUG, ORC_MSG_LEVEL_ERROR, ORC_MSG_LEVEL_FATAL, ORC_MSG_LEVEL_INFO,
    ORC_MSG_LEVEL_WARN, ORC_NUM_DIMS, ORC_OPAQUE, ORC_PROXY, ORC_U8, ORC_U16, ORC_U32, ORC_U64,
    OrcDims, OrcError,
};

mod error;
pub use error::Error;

#[cfg(feature = "derive")]
pub use orc_sdk_derive::{orc_fn, orc_fn_info, orc_generate_fn_info};

mod util;
pub use util::{
    FuncInfo, HandleDisplayWrapper, HostCallbacks, ObjectRegistry, PluginAllocator, PluginInfo,
    TypeInfo, handle_from_deck, ptr_from_slice, reset_handle, slice_from_ptr, slice_from_ptr_mut,
};
