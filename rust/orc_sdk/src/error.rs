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
    CannotLoadPlugins,
    OutOfBounds,
    AllocFailed,
    NullPointer,
    MissingCapability,
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
            crate::ORC_ERROR_CANNOT_LOAD_PLUGINS => Err(Error::CannotLoadPlugins),
            crate::ORC_ERROR_OUT_OF_BOUNDS => Err(Error::OutOfBounds),
            crate::ORC_ERROR_ALLOC_FAILED => Err(Error::AllocFailed),
            crate::ORC_ERROR_NULL_PTR => Err(Error::NullPointer),
            crate::ORC_ERROR_MISSING_CAPABILITY => Err(Error::MissingCapability),
            _ => Err(Error::Unknown),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidCombinations => write!(f, "Invalid combinations"),
            Error::InvalidHandle => write!(f, "Invalid handle"),
            Error::DeckTypeMismatch => write!(f, "Deck type mismatch"),
            Error::InvalidDimensions => write!(f, "Invalid dimensions"),
            Error::ConcurrencyProblem => write!(f, "Concurrency problem"),
            Error::PluginAlreadyInitialized => write!(f, "Plugin already initialized"),
            Error::ABIVersionMismatch => write!(f, "Abi version mismatch"),
            Error::InvalidProxy => write!(f, "Invalid proxy"),
            Error::CannotLoadPlugins => write!(f, "Cannot load plugins"),
            Error::OutOfBounds => write!(f, "Memory out of bounds"),
            Error::AllocFailed => write!(f, "Memory allocation failed"),
            Error::NullPointer => write!(f, "Null pointer"),
            Error::MissingCapability => write!(f, "This capability is missing / not implemented"),
            Error::Unknown => write!(f, "Unknown error"),
        }
    }
}

impl std::error::Error for Error {}

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
            Error::CannotLoadPlugins => crate::ORC_ERROR_CANNOT_LOAD_PLUGINS,
            Error::OutOfBounds => crate::ORC_ERROR_OUT_OF_BOUNDS,
            Error::AllocFailed => crate::ORC_ERROR_ALLOC_FAILED,
            Error::NullPointer => crate::ORC_ERROR_NULL_PTR,
            Error::MissingCapability => crate::ORC_ERROR_MISSING_CAPABILITY,
            Error::Unknown => crate::ORC_ERROR_UNKNOWN,
        }
    }
}
