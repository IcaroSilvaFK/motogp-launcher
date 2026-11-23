use crate::AppError;
use std::path::Path;

/// A direct child of the data folder with its recursive size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedEntry {
    pub name: String,
    pub is_dir: bool,
    pub bytes: u64,
}

/// Port: measures the data folder on disk.
pub trait StorageScanner {
    /// Every direct child of `root` with its recursive size. A missing root is
    /// an empty list; an unreadable root is an error.
    fn scan(&self, root: &Path) -> Result<Vec<ScannedEntry>, AppError>;

    /// Free space of the drive that holds `root`.
    fn free_space(&self, root: &Path) -> Result<u64, AppError>;
}

/// Port: shows a folder in the system file manager.
pub trait FolderOpener {
    fn open_folder(&self, path: &Path) -> Result<(), AppError>;
}

impl<T: FolderOpener + ?Sized> FolderOpener for std::sync::Arc<T> {
    fn open_folder(&self, path: &Path) -> Result<(), AppError> {
        (**self).open_folder(path)
    }
}
