//! Ports and narrow collaborator traits of the Play orchestrator. No Tauri,
//! threads or fs here: the interface and infrastructure implement them.
use crate::{AppError, DeployError, DeployMod, PlayError, SessionError, TrackOutcome};
use motogp_domain::{GameInfo, WatchEvent};
use std::path::Path;

/// Session state published to the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Started,
    Ended,
    NeverStarted,
}

/// What the interface hears about a session. `cleaned` is the number of
/// files actually removed (always 0 for `Started`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionEvent {
    pub state: SessionState,
    pub cleaned: usize,
}

/// Port: the single "session active" flag. Held from the Play request until
/// the cleanup finishes.
pub trait SessionLock: Send + Sync {
    /// `true` when the lock was free and is now held by the caller.
    fn try_acquire(&self) -> bool;
    fn release(&self);
}

/// Port: runs the (long) session tracking off the caller's thread.
pub trait BackgroundRunner: Send + Sync {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>);
}

/// Port: how session results reach the user interface.
pub trait SessionPublisher: Send + Sync {
    /// A session event (`started`, `ended`, `never-started`).
    fn publish(&self, event: SessionEvent);
    /// A failure that must be visible: it is never reported as a session event.
    fn report_error(&self, message: &str);
}

/// The configured game, validated by inspection.
pub trait PlayGame: Send + Sync {
    /// `PlayError::GameNotConfigured` when no folder was chosen.
    fn configured_game(&self) -> Result<GameInfo, PlayError>;
}

/// Turns a profile ID into the ordered mods to deploy.
pub trait ModPlanner: Send + Sync {
    fn plan(&self, profile_id: &str) -> Result<Vec<DeployMod>, PlayError>;
}

/// Deploy, clean and inspect the launcher's manifest.
pub trait Deployer: Send + Sync {
    fn deploy(&self, mods_dir: &Path, mods: &[DeployMod]) -> Result<Vec<String>, DeployError>;
    /// Removes what the launcher deployed; returns the number of files removed.
    fn clean(&self, mods_dir: &Path) -> Result<usize, DeployError>;
    /// `true` when the manifest tracks nothing.
    fn is_clean(&self) -> Result<bool, DeployError>;
}

/// Opens the game and follows its process.
pub trait SessionRuntime: Send + Sync {
    fn launch(&self, game: &GameInfo) -> Result<(), SessionError>;
    /// Blocks until a terminal event (calls `emit` for every event).
    fn track(
        &self,
        root: &Path,
        emit: &mut dyn FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError>;
}
