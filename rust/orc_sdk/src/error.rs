use crate::OrcError;

#[derive(Debug)]
pub enum Error {
    InvalidCombinations,
    InvalidHandle,
    DeckTypeMismatch,
    InvalidDimensions,
    ConcurrencyProblem,
    PluginAlreadyInitialized,
    ABIVersionMismatch,
    InvalidProxy,
    Unknown,
}

impl Error {
    pub fn from_raw(err: OrcError) -> Result<(), Error> {
        match err {
            crate::ORC_ERROR_NONE => Ok(()),
            crate::ORC_ERROR_INVALID_HANDLE => Err(Error::InvalidHandle),
            crate::ORC_ERROR_INVALID_DIMENSIONS => Err(Error::InvalidDimensions),
            crate::ORC_ERROR_TYPE_MISMATCH => Err(Error::DeckTypeMismatch),
            crate::ORC_ERROR_ABI_VERSION_MISMATCH => Err(Error::ABIVersionMismatch),
            crate::ORC_ERROR_PLUGIN_ALREADY_INITIALIZED => Err(Error::PluginAlreadyInitialized),
            crate::ORC_ERROR_INVALID_COMBINATIONS => Err(Error::InvalidCombinations),
            crate::ORC_ERROR_CONCURRENCY_PROBLEM => Err(Error::ConcurrencyProblem),
            crate::ORC_ERROR_INVALID_PROXY => Err(Error::InvalidProxy),
            _ => Err(Error::Unknown),
        }
    }
}

impl From<Error> for OrcError {
    fn from(value: Error) -> Self {
        match value {
            Error::InvalidHandle => crate::ORC_ERROR_INVALID_HANDLE,
            Error::InvalidDimensions => crate::ORC_ERROR_INVALID_DIMENSIONS,
            Error::DeckTypeMismatch => crate::ORC_ERROR_TYPE_MISMATCH,
            Error::ABIVersionMismatch => crate::ORC_ERROR_ABI_VERSION_MISMATCH,
            Error::PluginAlreadyInitialized => crate::ORC_ERROR_PLUGIN_ALREADY_INITIALIZED,
            Error::InvalidCombinations => crate::ORC_ERROR_INVALID_COMBINATIONS,
            Error::ConcurrencyProblem => crate::ORC_ERROR_CONCURRENCY_PROBLEM,
            Error::InvalidProxy => crate::ORC_ERROR_INVALID_PROXY,
            Error::Unknown => crate::ORC_ERROR_UNKNOWN,
        }
    }
}
