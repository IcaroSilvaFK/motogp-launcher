use crate::AppError;
use motogp_domain::AdSlot;
use std::fmt;

/// Why the ad source could not deliver the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdSourceError {
    /// The feed file does not exist on the server (HTTP 404): not published yet.
    NotPublished,
    /// The server could not be reached (DNS, connection, timeout, TLS).
    Network(String),
    /// The server answered with an unexpected HTTP status.
    Http { status: u16 },
    /// The response body is larger than the allowed `limit` in bytes.
    TooLarge { limit: u64 },
    /// The body is not an ad feed at all (not JSON, wrong shape).
    Invalid(String),
}

impl fmt::Display for AdSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdSourceError::NotPublished => write!(f, "ad feed not published"),
            AdSourceError::Network(detail) => write!(f, "network error: {detail}"),
            AdSourceError::Http { status } => write!(f, "unexpected HTTP status {status}"),
            AdSourceError::TooLarge { limit } => write!(f, "ad feed larger than {limit} bytes"),
            AdSourceError::Invalid(detail) => write!(f, "invalid ad feed: {detail}"),
        }
    }
}

impl std::error::Error for AdSourceError {}

/// The ad cache could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdCacheError(pub String);

impl fmt::Display for AdCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ad cache error: {}", self.0)
    }
}

impl std::error::Error for AdCacheError {}

/// The impression beacon could not be sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdBeaconError(pub String);

impl fmt::Display for AdBeaconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ad beacon error: {}", self.0)
    }
}

impl std::error::Error for AdBeaconError {}

/// Errors of the ad use cases. Problems with the feed itself are never errors:
/// they just mean there is no ad to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdError {
    /// The creative id is not currently offered for that slot.
    UnknownCreative { slot: AdSlot, id: String },
    /// The system could not open the click URL in the browser.
    Open(AppError),
}

impl fmt::Display for AdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdError::UnknownCreative { slot, id } => {
                write!(f, "creative {id} is not offered for slot {}", slot.as_str())
            }
            AdError::Open(error) => write!(f, "could not open the ad page: {error}"),
        }
    }
}

impl std::error::Error for AdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_readable_messages() {
        let source = [
            AdSourceError::NotPublished,
            AdSourceError::Network("down".into()),
            AdSourceError::Http { status: 500 },
            AdSourceError::TooLarge { limit: 10 },
            AdSourceError::Invalid("junk".into()),
        ];
        let text: Vec<String> = source.iter().map(|e| e.to_string()).collect();
        assert!(text[1].contains("down") && text[2].contains("500"));
        assert!(text[3].contains("10") && text[4].contains("junk"));
        assert!(text.iter().all(|t| !t.is_empty()));
        assert!(AdCacheError("x".into()).to_string().contains('x'));
        assert!(AdBeaconError("y".into()).to_string().contains('y'));
        let unknown = AdError::UnknownCreative {
            slot: AdSlot::About,
            id: "abc".into(),
        };
        assert!(unknown.to_string().contains("abc") && unknown.to_string().contains("about"));
        let open = AdError::Open(AppError::Read {
            location: "l".into(),
            detail: "no browser".into(),
        });
        assert!(open.to_string().contains("no browser"));
        let _: &dyn std::error::Error = &open;
    }
}
