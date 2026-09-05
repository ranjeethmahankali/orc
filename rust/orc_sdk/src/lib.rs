mod deck;
pub use deck::{Combinations, Deck, DeckView, DeckWriter};

mod ffi;
pub use ffi::{
    DeckAllocFn, DeckDeserializeFn, DeckFreeFn, DeckFromProxyFn, DeckSerializeFn, DeckToStringFn,
    OrcHandleBorrowed, PluginInitFn, ProxyType, TOrcData, TOrcPluginAdaptor, dims_divide,
    dims_multiply, dims_pow,
};

mod bindings;
pub use bindings::*;

mod error;
pub use error::Error;

#[cfg(feature = "derive")]
pub use orc_sdk_derive::{orc_fn, orc_fn_info, orc_map_fn};

mod util;
pub use util::{
    ContextArena, DeckRegistry, FuncInfo, HandleDisplayWrapper, HostCallbacks, PRIMITIVE_TYPES,
    PluginAllocator, SerialWrite, TypeInfo, deck_from_proxy, ptr_from_slice,
    read_orc_handle_header, reset_handle, slice_from_ptr, slice_from_ptr_mut, to_str_deck,
    try_deserialize_handle, try_serialize_handle, update_handle_from_deck, write_orc_handle_header,
};

mod dag;
pub use dag::{
    DagError, Handle as DagHandle, IH, InputPropBuf, InputProperty, LH, LinkPropBuf, LinkProperty,
    NH, NodeInfo, NodePropBuf, NodeProperty, OH, OutputPropBuf, OutputProperty, Property, Workflow,
};

mod host;
pub use host::{Plugin, PluginSet, TypeOwner};
