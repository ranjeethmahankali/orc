mod deck;
pub use deck::{Combinations, Deck, DeckView, DeckWriter};

mod ffi;
pub use ffi::{
    DeckAllocFn, DeckFreeFn, DeckFromProxyFn, PluginInitFn, ProxyType, TOrcData, TOrcPluginAdaptor,
    dims_divide, dims_multiply, dims_pow,
};

mod bindings;
pub use bindings::*;

mod error;
pub use error::Error;

#[cfg(feature = "derive")]
pub use orc_sdk_derive::{orc_fn, orc_fn_info, orc_generate_fn_info};

mod util;
pub use util::{
    BUILTIN_TYPES, DeckRegistry, FuncInfo, HandleDisplayWrapper, HostCallbacks, PluginAllocator,
    TypeInfo, ptr_from_slice, reset_handle, slice_from_ptr, slice_from_ptr_mut,
    update_handle_from_deck,
};

mod host;
pub use host::{Plugin, load_plugins};
