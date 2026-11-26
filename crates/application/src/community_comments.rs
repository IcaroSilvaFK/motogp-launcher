//! Ports and errors of the mod comments (issue #118). None of the errors ever
//! carries the session token.
use crate::CredentialError;
use motogp_domain::SessionToken;
use std::fmt;

/// A comment as the community server returns it (the avatar is still a URL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiComment {
    pub id: String,
    pub author_name: String,
    /// Absolute https URL of the author's avatar, if any.
    pub author_avatar_url: Option<String>,
    pub body: String,
    /// Milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    pub edited: bool,
    pub own: bool,
}

/// One page as the community server returns it, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiCommentsPage {
    pub comments: Vec<ApiComment>,
    pub next_cursor: Option<String>,
}

/// Why the community server could not serve a comments request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentsApiError {
    /// DNS, connection, timeout or TLS failure, or a 5xx answer.
    Unreachable(String),
    /// HTTP 401: the session is missing, expired or revoked.
    Unauthorized,
    /// HTTP 429, with the `Retry-After` seconds (1 when the header is absent).
    RateLimited { retry_after_secs: u64 },
    /// HTTP 400: the server refused the input.
    Invalid,
    /// HTTP 404: the comment does not exist or is not the caller's.
    NotFound,
    /// The server answered, but not with the documented contract.
    Protocol(String),
}

impl fmt::Display for CommentsApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommentsApiError::Unreachable(detail) => {
                write!(f, "the community server is unreachable: {detail}")
            }
            CommentsApiError::Unauthorized => write!(f, "the session is not valid"),
            CommentsApiError::RateLimited { retry_after_secs } => {
                write!(f, "too many comment writes, retry in {retry_after_secs}s")
            }
            CommentsApiError::Invalid => write!(f, "the server refused the input"),
            CommentsApiError::NotFound => write!(f, "the comment was not found"),
            CommentsApiError::Protocol(detail) => {
                write!(f, "unexpected community server answer: {detail}")
            }
        }
    }
}

impl std::error::Error for CommentsApiError {}

/// Port: the community server's comment endpoints.
pub trait CommunityCommentsApi: Send + Sync {
    /// One page, oldest first. The token (when given) only sets `own`; the
    /// server ignores a bad one.
    fn list(
        &self,
        mod_id: &str,
        cursor: Option<&str>,
        token: Option<&SessionToken>,
    ) -> Result<ApiCommentsPage, CommentsApiError>;
    fn post(
        &self,
        mod_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError>;
    fn edit(
        &self,
        mod_id: &str,
        comment_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError>;
    fn delete(
        &self,
        mod_id: &str,
        comment_id: &str,
        token: &SessionToken,
    ) -> Result<(), CommentsApiError>;
}

/// Errors of the comment use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentsError {
    /// The server could not be reached (or did not answer sensibly).
    Offline,
    /// A write needs a session and there is none (or the server rejected it).
    SignedOut,
    /// Too many writes; try again after `retry_after_secs`.
    RateLimited { retry_after_secs: u64 },
    /// The input breaks the rules (empty, too long, bad mod id...).
    Invalid,
    /// The comment is gone or is not the member's.
    NotFound,
    /// The credential store failed.
    Credential(CredentialError),
}

impl fmt::Display for CommentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommentsError::Offline => write!(f, "the community server is offline"),
            CommentsError::SignedOut => write!(f, "not signed in to the community"),
            CommentsError::RateLimited { retry_after_secs } => {
                write!(f, "too many comment writes, retry in {retry_after_secs}s")
            }
            CommentsError::Invalid => write!(f, "the comment is not valid"),
            CommentsError::NotFound => write!(f, "the comment was not found"),
            CommentsError::Credential(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CommentsError {}

impl From<CredentialError> for CommentsError {
    fn from(error: CredentialError) -> Self {
        CommentsError::Credential(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_readable_messages_without_secrets() {
        let api = [
            CommentsApiError::Unreachable("dns".into()),
            CommentsApiError::Unauthorized,
            CommentsApiError::RateLimited {
                retry_after_secs: 7,
            },
            CommentsApiError::Invalid,
            CommentsApiError::NotFound,
            CommentsApiError::Protocol("junk".into()),
        ];
        assert!(api.iter().all(|e| !e.to_string().is_empty()));
        assert!(api[0].to_string().contains("dns"));
        assert!(api[2].to_string().contains('7'));
        let app = [
            CommentsError::Offline,
            CommentsError::SignedOut,
            CommentsError::RateLimited {
                retry_after_secs: 9,
            },
            CommentsError::Invalid,
            CommentsError::NotFound,
            CommentsError::from(CredentialError("locked".into())),
        ];
        assert!(app.iter().all(|e| !e.to_string().is_empty()));
        assert!(app[2].to_string().contains('9'));
        assert!(app[5].to_string().contains("locked"));
        let _: &dyn std::error::Error = &app[0];
        let _: &dyn std::error::Error = &api[0];
    }
}
