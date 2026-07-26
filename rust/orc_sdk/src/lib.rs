mod deck;
pub use deck::{Combinations, Deck};

mod ffi;
pub use ffi::{ProxyType, TOrcData, TOrcPluginAdaptor, dims_divide, dims_multiply, dims_pow};

mod bindings;
pub use bindings::{OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin, OrcTypeId};
// Export all the type tags and other constants.
pub use bindings::{
    ORC_DECK_PROXY_COPY_ALL, ORC_DECK_PROXY_COPY_ITEMS, ORC_DECK_PROXY_SHUFFLE, ORC_DIM_CURRENT,
    ORC_DIM_LENGTH, ORC_DIM_LUMINOSITY, ORC_DIM_MASS, ORC_DIM_SUBSTANCE, ORC_DIM_TEMPERATURE,
    ORC_DIM_TIME, ORC_F32, ORC_F64, ORC_I8, ORC_I16, ORC_I32, ORC_I64, ORC_NUM_DIMS, ORC_OPAQUE,
    ORC_PROXY, ORC_U8, ORC_U16, ORC_U32, ORC_U64,
};

mod error;
pub use error::Error;

mod util;
pub use util::{ObjectRegistry, handle_from_deck, reset_handle};
