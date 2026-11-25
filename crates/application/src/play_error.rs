use crate::{DeployError, GameError, LibraryError, ProfileServiceError, SessionError};
use motogp_domain::LaunchError;
use std::fmt;

/// Errors of the Play use case and of the startup recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayError {
    /// A session is starting, running or still being cleaned up.
    AlreadyRunning,
    /// No game folder has been chosen yet.
    GameNotConfigured,
    /// The configured game could not be inspected.
    Game(GameError),
    /// The launch URL cannot be built (for example, no App ID).
    Launch(LaunchError),
    /// The profile could not be loaded.
    Profile(ProfileServiceError),
    /// The mod library could not be read.
    Library(LibraryError),
    /// Deploying or cleaning the mods failed.
    Deploy(DeployError),
    /// Opening Steam failed.
    Session(SessionError),
    /// A previous deployment could not be cleaned; new deploys stay blocked
    /// until it is resolved (or startup recovery has not run yet).
    RecoveryPending { detail: String },
    /// The game is still running, so its deployment is neither cleaned nor
    /// replaced until it ends.
    GameStillRunning,
    /// The process table could not be read, so it is unknown whether the
    /// game is running.
    ProcessInspection { detail: String },
    /// The background worker could not be started; the files stay deployed
    /// and the next Play retries the cleanup first.
    SessionThread { detail: String },
    /// Stop was requested but no session is starting or running.
    NoActiveSession,
    /// The game processes could not be ended; the session goes on.
    StopFailed { detail: String },
}

impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayError::AlreadyRunning => write!(f, "the game is already running"),
            PlayError::GameNotConfigured => write!(f, "choose the game folder first"),
            PlayError::Game(error) => write!(f, "{error}"),
            PlayError::Launch(error) => write!(f, "{error}"),
            PlayError::Profile(error) => write!(f, "{error}"),
            PlayError::Library(error) => write!(f, "{error}"),
            PlayError::Deploy(error) => write!(f, "{error}"),
            PlayError::Session(error) => write!(f, "{error}"),
            PlayError::RecoveryPending { detail } => {
                write!(f, "a previous session is not cleaned up yet: {detail}")
            }
            PlayError::GameStillRunning => {
                write!(
                    f,
                    "the game is still running; close it before playing again"
                )
            }
            PlayError::ProcessInspection { detail } => {
                write!(f, "could not check whether the game is running: {detail}")
            }
            PlayError::SessionThread { detail } => {
                write!(f, "could not follow the game session: {detail}")
            }
            PlayError::NoActiveSession => write!(f, "no active session"),
            PlayError::StopFailed { detail } => {
                write!(f, "could not stop the game: {detail}")
            }
        }
    }
}

impl std::error::Error for PlayError {}

impl From<GameError> for PlayError {
    fn from(error: GameError) -> Self {
        PlayError::Game(error)
    }
}

impl From<LaunchError> for PlayError {
    fn from(error: LaunchError) -> Self {
        PlayError::Launch(error)
    }
}

impl From<ProfileServiceError> for PlayError {
    fn from(error: ProfileServiceError) -> Self {
        PlayError::Profile(error)
    }
}

impl From<LibraryError> for PlayError {
    fn from(error: LibraryError) -> Self {
        PlayError::Library(error)
    }
}

impl From<DeployError> for PlayError {
    fn from(error: DeployError) -> Self {
        PlayError::Deploy(error)
    }
}

impl From<SessionError> for PlayError {
    fn from(error: SessionError) -> Self {
        PlayError::Session(error)
    }
}
