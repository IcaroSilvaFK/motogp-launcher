use crate::AppError;
use std::path::Path;
use std::time::Duration;

/// Interval between two process inspections while tracking a session.
pub const POLL: Duration = Duration::from_secs(2);

/// Port: opens a URL with the system handler (the `steam://` protocol).
pub trait UrlOpener {
    fn open(&self, url: &str) -> Result<(), AppError>;
}

/// Port: looks for a running game process.
pub trait ProcessInspector {
    /// True when a process whose executable lives inside `root` is running.
    fn game_running(&self, root: &Path) -> Result<bool, AppError>;
}

/// Port: monotonic time and waiting, so tests never sleep for real.
pub trait SessionClock {
    /// Time elapsed since an arbitrary, fixed origin.
    fn now(&self) -> Duration;
    /// Blocks (or advances a fake clock) for `duration`.
    fn sleep(&self, duration: Duration);
}

/// How a `track` call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackOutcome {
    /// The watcher reached a terminal game event (`Ended` / `NeverStarted`).
    Finished,
    /// The caller cancelled the tracking; no game event was fabricated.
    Cancelled,
}
