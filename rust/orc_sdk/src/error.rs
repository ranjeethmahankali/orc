use crate::{ORC_ERROR_INVALID_HANDLE, ORC_ERROR_TYPE_MISMATCH, ORC_ERROR_UNKNOWN};

#[derive(Debug)]
pub enum Error {
    ArrayLengthMismatch(usize, usize),
    InvalidHandle,
    DeckTypeMismatch,
    CannotLockRegistry,
    DeckBorrowError,
    PluginInitError,
}

impl Error {
    pub fn to_orc_error(&self) -> u32 {
        match self {
            Error::InvalidHandle => ORC_ERROR_INVALID_HANDLE,
            Error::DeckTypeMismatch => ORC_ERROR_TYPE_MISMATCH,
            Error::ArrayLengthMismatch(_, _)
            | Error::CannotLockRegistry
            | Error::PluginInitError
            | Error::DeckBorrowError => ORC_ERROR_UNKNOWN,
        }
    }
}
