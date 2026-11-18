use crate::AppError;
use motogp_domain::SteamManifest;
use std::path::{Path, PathBuf};

/// Port: where Steam keeps its game libraries.
pub trait SteamLocator {
    /// Every Steam library root (the folder containing `steamapps`).
    /// Steam not installed or not found yields an empty list, not an error.
    fn library_folders(&self) -> Result<Vec<PathBuf>, AppError>;
}

/// Port: read-only view of a game installation on disk. Never writes.
pub trait GameFiles {
    /// Immediate subfolders of `<library>/steamapps/common` (empty if missing).
    fn common_dirs(&self, library: &Path) -> Result<Vec<PathBuf>, AppError>;
    /// Names of the immediate subfolders of `dir` (empty if missing).
    fn subdirs(&self, dir: &Path) -> Result<Vec<String>, AppError>;
    /// Whether `<project_dir>/Content/Paks` exists.
    fn has_paks(&self, project_dir: &Path) -> bool;
    /// Manifests (`appmanifest_*.acf`) found in a `steamapps` folder.
    fn manifests(&self, steamapps: &Path) -> Result<Vec<SteamManifest>, AppError>;
}
