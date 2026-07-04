mod deck;
pub use deck::{Combinations, Deck};

mod ffi;
pub use ffi::{
    ORC_NUM_DIMS, OrcDims, OrcFuncInfo, OrcHandle, OrcHost, OrcItemProxy, OrcMark, OrcPlugin,
    OrcPrimitiveTypeId, OrcTypeInfo, TOrcPlugin,
};

mod error;
pub use error::Error;
