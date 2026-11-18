use crate::LibraryError;
use motogp_domain::ModEntry;
use std::path::{Path, PathBuf};

/// Port: the library index (`library.json`).
pub trait ModRepository {
    fn list(&self) -> Result<Vec<ModEntry>, LibraryError>;
    /// Registers a completed import.
    fn add(&self, entry: &ModEntry) -> Result<(), LibraryError>;
    /// Removes the entry; `true` when it existed.
    fn remove(&self, id: &str) -> Result<bool, LibraryError>;
}

/// Port: files of the library (`library/<id>/...`) and staging areas.
pub trait LibraryFiles {
    /// Names (not paths) of the files directly inside `dir`.
    fn list_dir(&self, dir: &Path) -> Result<Vec<String>, LibraryError>;
    /// Creates an empty temporary directory inside the library.
    fn create_staging(&self) -> Result<PathBuf, LibraryError>;
    /// Copies `source` into `dest_dir` keeping its file name.
    fn copy_file(&self, source: &Path, dest_dir: &Path) -> Result<(), LibraryError>;
    fn file_size(&self, file: &Path) -> Result<u64, LibraryError>;
    /// Moves the named files of `staging` into `library/<id>/`.
    fn publish(&self, staging: &Path, files: &[String], id: &str) -> Result<(), LibraryError>;
    /// Deletes a staging directory (best effort for the caller).
    fn discard(&self, staging: &Path) -> Result<(), LibraryError>;
    /// Deletes `library/<id>/`; a missing folder is not an error.
    fn remove_mod_dir(&self, id: &str) -> Result<(), LibraryError>;
}

/// Port: reads an archive (ZIP).
pub trait ArchiveReader {
    /// Extracts the mod files (`.pak/.utoc/.ucas`) of `archive` flattened into
    /// `dest_dir`, ignoring unsafe entries, and returns the written file names.
    fn extract_flat(&self, archive: &Path, dest_dir: &Path) -> Result<Vec<String>, LibraryError>;
}

/// Port: content hash.
pub trait ContentHasher {
    /// SHA-256 of the file content as lowercase hex.
    fn sha256_hex(&self, file: &Path) -> Result<String, LibraryError>;
}
