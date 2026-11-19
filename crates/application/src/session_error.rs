use crate::AppError;
use motogp_domain::LaunchError;
use std::fmt;

/// Errors of the launch use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The launch URL could not be built.
    Launch(LaunchError),
    /// The URL opener failed.
    Opener(AppError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Launch(error) => write!(f, "{error}"),
            SessionError::Opener(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<LaunchError> for SessionError {
    fn from(error: LaunchError) -> Self {
        SessionError::Launch(error)
    }
}
