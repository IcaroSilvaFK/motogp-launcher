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

/// Result of comparing the manifest's directory with the authorized `~mods`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirCheck {
    /// The manifest directory does not exist: there is nothing to clean.
    Missing,
    /// It resolves (after canonicalization, links and `..` included) to the
    /// authorized directory.
    Authorized,
    /// It resolves anywhere else.
    Foreign,
}

/// Port: the game `~mods` folder.
pub trait DeployFiles {
    /// Compares the manifest's directory with the independently authorized
    /// `~mods` after canonicalizing both (resolving `..`, symlinks and
    /// junctions). Never trusts the manifest's own claim.
    fn check_dir(&self, manifest_dir: &Path, authorized: &Path) -> Result<DirCheck, DeployError>;
    /// Ownership proof: `true` only when `dir/name` exists and is verifiably
    /// the launcher's file, i.e. the same file as the library `source` (hard
    /// link) or a byte-identical copy of it. Missing, different, non-regular
    /// files and an unreadable `source` yield `false`.
    fn owns(&self, dir: &Path, name: &str, source: &Path) -> Result<bool, DeployError>;
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
