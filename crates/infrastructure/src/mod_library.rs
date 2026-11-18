//! Composition of the mod library use cases with the real adapters.
use crate::{DataDir, FsLibraryFiles, JsonModRepository, Sha256Hasher, ZipArchiveReader};
use motogp_application::ModService;

/// `ModService` wired with the fs, JSON, ZIP and SHA-256 adapters.
pub type ModLibrary = ModService<JsonModRepository, FsLibraryFiles, ZipArchiveReader, Sha256Hasher>;

/// Builds the mod library rooted at the application data directory.
pub fn mod_library(data: &DataDir) -> ModLibrary {
    ModService::new(
        JsonModRepository::new(data.library_file()),
        FsLibraryFiles::new(data.library_dir()),
        ZipArchiveReader,
        Sha256Hasher,
    )
}
