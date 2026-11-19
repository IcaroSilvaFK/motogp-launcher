use crate::AppError;
use std::fmt;
use std::path::PathBuf;

/// Errors of the game detection use cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    /// The folder has no `<Project>/Content/Paks` (not an Unreal game).
    NotUnrealGame { root: PathBuf },
    /// A storage adapter failed while reading the installation.
    Storage(AppError),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::NotUnrealGame { root } => write!(
                f,
                "not an Unreal game: Content/Paks not found in {}",
                root.display()
            ),
            GameError::Storage(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GameError {}

impl From<AppError> for GameError {
    fn from(error: AppError) -> Self {
        GameError::Storage(error)
    }
}
