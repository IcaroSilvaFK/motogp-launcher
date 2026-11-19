use crate::AppError;
use std::fmt;

/// Errors of the deploy/clean use cases. Adapters translate their failures
/// (fs, JSON) into these variants; no I/O type leaks through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployError {
    /// Reading or writing the manifest failed.
    Storage(AppError),
    /// A file-system operation failed.
    Io { location: String, detail: String },
    /// The destination exists; it is never overwritten.
    AlreadyExists { name: String },
    /// A name or ID is not a simple relative name, or the manifest is unusable.
    InvalidPath { path: String },
    /// The manifest points at a directory other than the authorized `~mods`.
    UnauthorizedDirectory { path: String },
    /// Some files could not be removed; they remain tracked for a retry.
    CleanIncomplete {
        removed: usize,
        remaining: usize,
        detail: String,
    },
}

impl From<AppError> for DeployError {
    fn from(error: AppError) -> Self {
        DeployError::Storage(error)
    }
}

impl fmt::Display for DeployError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeployError::Storage(error) => write!(f, "{error}"),
            DeployError::Io { location, detail } => write!(f, "{location}: {detail}"),
            DeployError::AlreadyExists { name } => {
                write!(f, "the destination {name} already exists and was not overwritten")
            }
            DeployError::InvalidPath { path } => write!(f, "invalid path: {path:?}"),
            DeployError::UnauthorizedDirectory { path } => write!(
                f,
                "{path:?} is not the authorized mods directory; nothing was removed"
            ),
            DeployError::CleanIncomplete {
                removed,
                remaining,
                detail,
            } => write!(
                f,
                "cleanup incomplete: {removed} removed, {remaining} file(s) remain tracked for a retry ({detail})"
            ),
        }
    }
}

impl std::error::Error for DeployError {}
