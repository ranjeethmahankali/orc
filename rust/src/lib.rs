mod deck;
pub use deck::{Combinations, Deck};

mod ffi;
pub use ffi::{
    Dims, ORC_NUM_DIMS, OrcFuncInfo, OrcHandle, OrcHost, OrcMark, OrcPlugin, OrcPrimitiveTypeId,
    OrcTypeInfo, TOrcPlugin,
};

mod error;
pub use error::Error;
