mod deck;
pub use deck::{Combinations, Deck};

mod ffi;
pub use ffi::{
    TOrcPlugin, create_proxy_deck_primitive_type, dims_divide, dims_multiply, dims_pow,
    proxy_deck_view_from_handle,
};

mod bindings;

mod error;
pub use error::Error;
