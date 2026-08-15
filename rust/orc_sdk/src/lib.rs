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
pub use orc_sdk_derive::{orc_fn, orc_fn_info, orc_map_fn};

mod util;
pub use util::{
    DeckRegistry, FuncInfo, HandleDisplayWrapper, HostCallbacks, PRIMITIVE_TYPES, PluginAllocator,
    TypeInfo, deck_from_proxy, ptr_from_slice, reset_handle, slice_from_ptr, slice_from_ptr_mut,
    update_handle_from_deck,
};

mod dag;
pub use dag::{
    DagError, IH, InputPropBuf, InputProperty, LH, LinkPropBuf, LinkProperty, NH, NodeInfo,
    NodePropBuf, NodeProperty, OH, OutputPropBuf, OutputProperty, Property, Workflow,
};

mod host;
pub use host::{Plugin, PluginSet, TypeOwner, load_plugins};
