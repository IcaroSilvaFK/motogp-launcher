//! Multi-step library use cases: batch import and mod removal.
use crate::{
    ArchiveReader, Clock, ContentHasher, LibraryError, LibraryFiles, ModRepository, ModService,
    ProfileRepository, ProfileService, ProfileServiceError,
};
use motogp_domain::ModEntry;
use std::fmt;
use std::path::PathBuf;

/// A batch import that stopped at its first failure. Imports completed before
/// the failure stay in the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportBatchError {
    /// Entries registered before the failure (also visible after a reload).
    pub completed: Vec<ModEntry>,
    /// The path that failed; later paths were not attempted.
    pub failed: PathBuf,
    pub error: LibraryError,
}

impl fmt::Display for ImportBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "could not import {}: {} ({} import(s) completed before the failure)",
            self.failed.display(),
            self.error,
            self.completed.len()
        )
    }
}

impl std::error::Error for ImportBatchError {}

/// Imports each path in order, stopping at the first error.
pub fn import_mods<R, F, A, H>(
    mods: &ModService<R, F, A, H>,
    paths: &[PathBuf],
) -> Result<Vec<ModEntry>, ImportBatchError>
where
    R: ModRepository,
    F: LibraryFiles,
    A: ArchiveReader,
    H: ContentHasher,
{
    let mut completed: Vec<ModEntry> = Vec::new();
    for path in paths {
        match mods.import(path) {
            Ok(entries) => {
                for entry in entries {
                    if completed.iter().all(|known| known.id != entry.id) {
                        completed.push(entry);
                    }
                }
            }
            Err(error) => {
                return Err(ImportBatchError {
                    completed,
                    failed: path.clone(),
                    error,
                })
            }
        }
    }
    Ok(completed)
}

/// Why removing a mod did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoveModError {
    /// A game session is active: the library and the profiles were left untouched.
    SessionActive,
    /// The library could not remove the mod; profiles were left untouched.
    Library(LibraryError),
    /// The mod was removed but a profile could not be updated.
    Profiles(ProfileServiceError),
}

impl fmt::Display for RemoveModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoveModError::SessionActive => write!(
                f,
                "mods cannot be removed from the library while a game session is active"
            ),
            RemoveModError::Library(error) => write!(f, "{error}"),
            RemoveModError::Profiles(error) => write!(
                f,
                "the mod was removed but its references could not be cleared: {error}"
            ),
        }
    }
}

impl std::error::Error for RemoveModError {}

/// Removes the mod from the library, then its references from every profile.
/// A mod that is already gone still gets its stale references cleared, and
/// the not-found error is reported.
pub fn remove_mod<R, F, A, H, PR, C>(
    mods: &ModService<R, F, A, H>,
    profiles: &ProfileService<PR, C>,
    id: &str,
) -> Result<(), RemoveModError>
where
    R: ModRepository,
    F: LibraryFiles,
    A: ArchiveReader,
    H: ContentHasher,
    PR: ProfileRepository,
    C: Clock,
{
    // The lock is checked first so a refused removal leaves everything untouched.
    profiles
        .ensure_idle()
        .map_err(|_| RemoveModError::SessionActive)?;
    match mods.remove(id) {
        Ok(()) => profiles
            .remove_mod_everywhere(id)
            .map_err(RemoveModError::Profiles),
        Err(error @ LibraryError::NotFound { .. }) => {
            profiles
                .remove_mod_everywhere(id)
                .map_err(RemoveModError::Profiles)?;
            Err(RemoveModError::Library(error))
        }
        Err(error) => Err(RemoveModError::Library(error)),
    }
}
