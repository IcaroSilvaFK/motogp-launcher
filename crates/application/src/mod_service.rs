use crate::{ArchiveReader, ContentHasher, LibraryError, LibraryFiles, ModRepository};
use motogp_domain::ModEntry;
use std::path::Path;

/// Use cases of the local mod library: list, import and remove.
pub struct ModService<R, F, A, H> {
    repository: R,
    files: F,
    archive: A,
    hasher: H,
}

impl<R, F, A, H> ModService<R, F, A, H>
where
    R: ModRepository,
    F: LibraryFiles,
    A: ArchiveReader,
    H: ContentHasher,
{
    pub fn new(repository: R, files: F, archive: A, hasher: H) -> Self {
        Self {
            repository,
            files,
            archive,
            hasher,
        }
    }

    pub fn list(&self) -> Result<Vec<ModEntry>, LibraryError> {
        let _ = (&self.repository, &self.files, &self.archive, &self.hasher);
        todo!()
    }

    /// Imports a `.pak/.utoc/.ucas` (only its same-name siblings) or a `.zip`
    /// (every valid group). Re-importing the same content returns the existing entry.
    pub fn import(&self, _source: &Path) -> Result<Vec<ModEntry>, LibraryError> {
        todo!()
    }

    /// Removes the mod folder and its index entry.
    pub fn remove(&self, _id: &str) -> Result<(), LibraryError> {
        todo!()
    }
}
