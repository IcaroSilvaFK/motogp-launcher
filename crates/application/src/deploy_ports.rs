use crate::DeployError;
use motogp_domain::Deployed;
use std::path::Path;

/// Port: the deployment manifest (`deployed.json`).
pub trait DeployedRepository {
    /// Missing storage yields `Deployed::default()`; invalid storage is an error.
    fn load(&self) -> Result<Deployed, DeployError>;
    /// Persists the manifest durably before returning.
    fn save(&self, deployed: &Deployed) -> Result<(), DeployError>;
}

/// Port: the game `~mods` folder.
pub trait DeployFiles {
    /// Whether `dir/name` exists (a missing `dir` means `false`).
    fn exists(&self, dir: &Path, name: &str) -> Result<bool, DeployError>;
    fn create_dir_all(&self, dir: &Path) -> Result<(), DeployError>;
    /// Creates `dir/name` from `source` (hard link, copy as a fallback) with
    /// exclusive creation: an existing destination is `AlreadyExists`, never
    /// overwritten, and a failure leaves no partial destination behind.
    fn materialize(&self, source: &Path, dir: &Path, name: &str) -> Result<(), DeployError>;
    /// Removes `dir/name`; `false` when it was already missing.
    fn remove(&self, dir: &Path, name: &str) -> Result<bool, DeployError>;
}
