use std::path::PathBuf;

/// What the deployment manifest says about the current game install, as seen
/// by [`DeployService::session`](crate::DeployService::session) (read-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeploySession {
    /// Nothing tracked.
    None,
    /// The manifest points at the authorized `~mods`: the normal clean/deploy apply.
    Current { tracked: usize },
    /// The manifest points at another existing folder (the game install was
    /// switched): a recovery action is needed.
    Stale { mods_dir: PathBuf, tracked: usize },
    /// The recorded folder no longer exists: the next clean/deploy drops the manifest.
    Vanished { mods_dir: PathBuf },
}
