//! Errors of the community account (issue #122). None of them ever carries the
//! session token, the one-time code or the PKCE verifier.
use std::fmt;

/// Why the community server could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunityApiError {
    /// HTTP 401: the session is missing, expired or revoked.
    Unauthorized,
    /// DNS, connection, timeout or TLS failure, or a 5xx answer.
    Unreachable(String),
    /// The server answered, but not with the documented contract
    /// (including 400 `invalid_code`).
    Protocol(String),
}

impl fmt::Display for CommunityApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommunityApiError::Unauthorized => write!(f, "the session is not valid"),
            CommunityApiError::Unreachable(detail) => {
                write!(f, "the community server is unreachable: {detail}")
            }
            CommunityApiError::Protocol(detail) => {
                write!(f, "unexpected community server answer: {detail}")
            }
        }
    }
}

impl std::error::Error for CommunityApiError {}

/// The credential store could not read, write or delete the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialError(pub String);

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "credential store error: {}", self.0)
    }
}

impl std::error::Error for CredentialError {}

/// The loopback listener could not receive the browser callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopbackError {
    /// Nobody came back before the deadline.
    Timeout,
    /// The socket failed.
    Io(String),
}

impl fmt::Display for LoopbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoopbackError::Timeout => write!(f, "timed out waiting for the browser"),
            LoopbackError::Io(detail) => write!(f, "loopback error: {detail}"),
        }
    }
}

impl std::error::Error for LoopbackError {}

/// Errors of the community use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunityError {
    /// The server is unreachable and the action needed it.
    Offline,
    /// Sign-in did not complete (bad callback, state mismatch, server refusal).
    SignInFailed,
    /// The browser never came back in time.
    SignInTimeout,
    /// The user refused access on Discord.
    SignInDenied,
    /// The credential store failed.
    Credential(CredentialError),
}

impl fmt::Display for CommunityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommunityError::Offline => write!(f, "the community server is offline"),
            CommunityError::SignInFailed => write!(f, "sign-in failed"),
            CommunityError::SignInTimeout => write!(f, "sign-in timed out"),
            CommunityError::SignInDenied => write!(f, "sign-in was denied"),
            CommunityError::Credential(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CommunityError {}

impl From<CredentialError> for CommunityError {
    fn from(error: CredentialError) -> Self {
        CommunityError::Credential(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_readable_messages() {
        assert!(CommunityApiError::Unreachable("dns".into())
            .to_string()
            .contains("dns"));
        assert!(CommunityApiError::Protocol("junk".into())
            .to_string()
            .contains("junk"));
        assert!(!CommunityApiError::Unauthorized.to_string().is_empty());
        assert!(LoopbackError::Io("bind".into())
            .to_string()
            .contains("bind"));
        assert!(!LoopbackError::Timeout.to_string().is_empty());
        let all = [
            CommunityError::Offline,
            CommunityError::SignInFailed,
            CommunityError::SignInTimeout,
            CommunityError::SignInDenied,
            CommunityError::from(CredentialError("locked".into())),
        ];
        assert!(all.iter().all(|e| !e.to_string().is_empty()));
        assert!(all[4].to_string().contains("locked"));
        let _: &dyn std::error::Error = &all[0];
    }
}
