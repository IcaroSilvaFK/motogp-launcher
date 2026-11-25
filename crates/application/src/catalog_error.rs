use std::fmt;

/// Why the online catalog source could not deliver a catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogSourceError {
    /// The server could not be reached (DNS, connection, timeout, TLS).
    Network(String),
    /// The server answered with an unexpected HTTP status.
    Http { status: u16 },
    /// The server asked us to slow down (HTTP 429/503), optionally for how long.
    RateLimited { retry_after_secs: Option<u64> },
    /// The response body is larger than the allowed `limit` in bytes.
    TooLarge { limit: u64 },
    /// The body is not a catalog document at all (not JSON, wrong shape).
    Invalid(String),
}

impl fmt::Display for CatalogSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogSourceError::Network(detail) => write!(f, "network error: {detail}"),
            CatalogSourceError::Http { status } => write!(f, "unexpected HTTP status {status}"),
            CatalogSourceError::RateLimited {
                retry_after_secs: Some(secs),
            } => write!(f, "rate limited, retry after {secs} s"),
            CatalogSourceError::RateLimited {
                retry_after_secs: None,
            } => write!(f, "rate limited"),
            CatalogSourceError::TooLarge { limit } => {
                write!(f, "catalog larger than {limit} bytes")
            }
            CatalogSourceError::Invalid(detail) => write!(f, "invalid catalog: {detail}"),
        }
    }
}

impl std::error::Error for CatalogSourceError {}

/// The catalog cache could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCacheError(pub String);

impl fmt::Display for CatalogCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "catalog cache error: {}", self.0)
    }
}

impl std::error::Error for CatalogCacheError {}

/// Errors of the catalog use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    /// No catalog can be shown: nothing cached and the source failed.
    Unavailable { reason: String },
    /// The catalog file uses a newer schema than this launcher understands.
    UnsupportedSchema { version: u32 },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::Unavailable { reason } => write!(f, "catalog unavailable: {reason}"),
            CatalogError::UnsupportedSchema { version } => {
                write!(f, "unsupported catalog schema version {version}")
            }
        }
    }
}

impl std::error::Error for CatalogError {}
