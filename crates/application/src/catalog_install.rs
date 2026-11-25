//! Use case: install a catalog entry (issue #74). Two steps so the long
//! download never holds the application write lock: `prepare` downloads and
//! verifies into a staging area, `install` imports the staged file.
use crate::{
    ArchiveReader, CancelFlag, CatalogCache, CatalogError, CatalogService, CatalogSource, Clock,
    ContentHasher, DownloadError, DownloadProgress, DownloadRequest, DownloadStaging, LibraryError,
    LibraryFiles, ModDownloader, ModRepository, ModService, ProfileRepository, ProfileService,
};
use motogp_domain::{CatalogDownload, ModEntry};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Which step of an install is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    Downloading,
    Verifying,
}

/// Progress of one install, reported by `prepare`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallProgress {
    pub phase: InstallPhase,
    pub received: u64,
    pub total: u64,
}

/// Why an install did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogInstallError {
    /// A game session is active: nothing was downloaded or changed.
    SessionActive,
    /// No catalog entry has this id.
    EntryNotFound { id: String },
    /// The entry only links to a page: the author does not allow a direct download.
    NotDownloadable { id: String },
    /// This entry is already being installed.
    InstallInProgress { id: String },
    /// The server could not be reached or answered with an error.
    Network(String),
    /// The file size differs from the catalog. `actual` is the real size, or
    /// one more than `expected` when the download was cut for exceeding it.
    SizeMismatch { expected: u64, actual: u64 },
    /// The SHA-256 of the file differs from the catalog.
    HashMismatch,
    /// The user cancelled the install.
    Cancelled,
    /// The catalog could not be read.
    Catalog(CatalogError),
    /// Hashing or importing into the library failed.
    Library(LibraryError),
    /// The staging area could not be used.
    Storage(String),
}

impl fmt::Display for CatalogInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogInstallError::SessionActive => {
                write!(f, "mods cannot be installed while a game session is active")
            }
            CatalogInstallError::EntryNotFound { id } => {
                write!(f, "no catalog entry with id \"{id}\"")
            }
            CatalogInstallError::NotDownloadable { id } => {
                write!(f, "catalog entry \"{id}\" has no direct download")
            }
            CatalogInstallError::InstallInProgress { id } => {
                write!(f, "catalog entry \"{id}\" is already being installed")
            }
            CatalogInstallError::Network(detail) => write!(f, "download failed: {detail}"),
            CatalogInstallError::SizeMismatch { expected, actual } => write!(
                f,
                "downloaded size {actual} does not match the catalog size {expected}"
            ),
            CatalogInstallError::HashMismatch => {
                write!(f, "the downloaded file does not match the catalog SHA-256")
            }
            CatalogInstallError::Cancelled => write!(f, "install cancelled"),
            CatalogInstallError::Catalog(error) => write!(f, "{error}"),
            CatalogInstallError::Library(error) => write!(f, "{error}"),
            CatalogInstallError::Storage(detail) => write!(f, "install storage error: {detail}"),
        }
    }
}

impl std::error::Error for CatalogInstallError {}

type InFlight = Arc<Mutex<HashSet<String>>>;

/// Marks an entry as being installed until dropped.
#[derive(Debug)]
struct InFlightGuard {
    set: InFlight,
    id: String,
}

impl InFlightGuard {
    fn acquire(set: &InFlight, id: &str) -> Option<Self> {
        let inserted = set
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.to_string());
        inserted.then(|| Self {
            set: set.clone(),
            id: id.to_string(),
        })
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.set
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// A downloaded and verified file, ready to be imported. Holds the
/// "one install per entry" slot until it is installed or dropped.
#[derive(Debug)]
pub struct StagedDownload {
    entry_id: String,
    path: PathBuf,
    size: u64,
    _guard: InFlightGuard,
}

impl StagedDownload {
    pub fn entry_id(&self) -> &str {
        &self.entry_id
    }

    /// Size in bytes of the verified file.
    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Downloads catalog entries and hands them to the library.
pub struct CatalogInstaller<D, S, H> {
    downloader: D,
    staging: S,
    hasher: H,
    in_flight: InFlight,
}

impl<D, S, H> CatalogInstaller<D, S, H>
where
    D: ModDownloader,
    S: DownloadStaging,
    H: ContentHasher,
{
    pub fn new(downloader: D, staging: S, hasher: H) -> Self {
        Self {
            downloader,
            staging,
            hasher,
            in_flight: Arc::default(),
        }
    }

    /// Downloads the entry `id` into the staging area and verifies size and
    /// SHA-256. The staging area is removed again on every error.
    pub fn prepare<CS, CC, K, PR, PC>(
        &self,
        catalog: &CatalogService<CS, CC, K>,
        profiles: &ProfileService<PR, PC>,
        id: &str,
        progress: &mut dyn FnMut(InstallProgress),
        cancel: &CancelFlag,
    ) -> Result<StagedDownload, CatalogInstallError>
    where
        CS: CatalogSource,
        CC: CatalogCache,
        K: Clock,
        PR: ProfileRepository,
        PC: Clock,
    {
        ensure_idle(profiles)?;
        let snapshot = catalog.get().map_err(CatalogInstallError::Catalog)?;
        let entry = snapshot
            .entries
            .into_iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| CatalogInstallError::EntryNotFound { id: id.to_string() })?;
        let download = entry
            .download
            .ok_or_else(|| CatalogInstallError::NotDownloadable { id: id.to_string() })?;
        let guard = InFlightGuard::acquire(&self.in_flight, id)
            .ok_or_else(|| CatalogInstallError::InstallInProgress { id: id.to_string() })?;

        let dir = self
            .staging
            .create(id)
            .map_err(map_download_error(&download))?;
        match self.download_and_verify(&dir, id, &download, progress, cancel) {
            Ok(path) => Ok(StagedDownload {
                entry_id: id.to_string(),
                path,
                size: download.size,
                _guard: guard,
            }),
            Err(error) => {
                self.staging.remove(id);
                Err(error)
            }
        }
    }

    fn download_and_verify(
        &self,
        dir: &Path,
        id: &str,
        download: &CatalogDownload,
        progress: &mut dyn FnMut(InstallProgress),
        cancel: &CancelFlag,
    ) -> Result<PathBuf, CatalogInstallError> {
        let name = file_name_for(&download.url, id);
        let part = dir.join(format!("{name}.part"));
        let request = DownloadRequest {
            url: download.url.clone(),
            max_bytes: download.size,
        };
        let total = download.size;
        let mut on_download = |p: DownloadProgress| {
            progress(InstallProgress {
                phase: InstallPhase::Downloading,
                received: p.received,
                total: p.total,
            })
        };
        self.downloader
            .download(&request, &part, &mut on_download, cancel)
            .map_err(map_download_error(download))?;

        let actual = self
            .staging
            .file_size(&part)
            .map_err(map_download_error(download))?;
        if actual != download.size {
            return Err(CatalogInstallError::SizeMismatch {
                expected: download.size,
                actual,
            });
        }
        progress(InstallProgress {
            phase: InstallPhase::Verifying,
            received: total,
            total,
        });
        let digest = self
            .hasher
            .sha256_hex(&part)
            .map_err(CatalogInstallError::Library)?;
        if !digest.eq_ignore_ascii_case(&download.sha256) {
            return Err(CatalogInstallError::HashMismatch);
        }
        let final_path = dir.join(&name);
        self.staging
            .rename(&part, &final_path)
            .map_err(map_download_error(download))?;
        Ok(final_path)
    }

    /// Imports the staged file into the library. Re-checks the session lock
    /// and always removes the staging area.
    pub fn install<R, F, A, MH, PR, PC>(
        &self,
        mods: &ModService<R, F, A, MH>,
        profiles: &ProfileService<PR, PC>,
        staged: StagedDownload,
    ) -> Result<Vec<ModEntry>, CatalogInstallError>
    where
        R: ModRepository,
        F: LibraryFiles,
        A: ArchiveReader,
        MH: ContentHasher,
        PR: ProfileRepository,
        PC: Clock,
    {
        let result = ensure_idle(profiles).and_then(|()| {
            mods.import(staged.path())
                .map_err(CatalogInstallError::Library)
        });
        self.discard(staged);
        result
    }

    /// Drops a staged download without importing it.
    pub fn discard(&self, staged: StagedDownload) {
        self.staging.remove(staged.entry_id());
    }
}

fn ensure_idle<PR, PC>(profiles: &ProfileService<PR, PC>) -> Result<(), CatalogInstallError>
where
    PR: ProfileRepository,
    PC: Clock,
{
    profiles
        .ensure_idle()
        .map_err(|_| CatalogInstallError::SessionActive)
}

fn map_download_error(download: &CatalogDownload) -> impl Fn(DownloadError) -> CatalogInstallError {
    let expected = download.size;
    move |error| match error {
        DownloadError::Network(detail) => CatalogInstallError::Network(detail),
        DownloadError::TooLarge { .. } => CatalogInstallError::SizeMismatch {
            expected,
            actual: expected.saturating_add(1),
        },
        DownloadError::Cancelled => CatalogInstallError::Cancelled,
        DownloadError::Storage(detail) => CatalogInstallError::Storage(detail),
    }
}

/// File name of a download: the last URL path segment, made safe. Keeps the
/// extension (the importer picks its reader by it); falls back to `<id>.zip`.
fn file_name_for(url: &str, id: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or("");
    let without_query = without_fragment.split('?').next().unwrap_or("");
    let segment = without_query.rsplit('/').next().unwrap_or("");
    let safe: String = segment
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim_matches('.');
    if safe.is_empty() || safe.contains("..") {
        format!("{id}.zip")
    } else {
        safe.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_is_the_last_url_segment() {
        assert_eq!(file_name_for("https://x.io/a/b/mod.zip", "id"), "mod.zip");
        assert_eq!(
            file_name_for("https://x.io/mod.pak?dl=1#f", "id"),
            "mod.pak"
        );
    }

    #[test]
    fn file_name_is_sanitized() {
        assert_eq!(
            file_name_for("https://x.io/my%20mod.zip", "id"),
            "my_20mod.zip"
        );
        assert_eq!(file_name_for("https://x.io/a\\b.zip", "id"), "a_b.zip");
        assert_eq!(file_name_for("https://x.io/a..b.zip", "id"), "id.zip");
    }

    #[test]
    fn file_name_falls_back_to_the_entry_id() {
        assert_eq!(
            file_name_for("https://x.io/", "rebalanced-ai"),
            "rebalanced-ai.zip"
        );
        assert_eq!(file_name_for("https://x.io/..", "abc"), "abc.zip");
        assert_eq!(file_name_for("https://x.io/%%%", "abc"), "___");
    }

    #[test]
    fn install_errors_have_english_messages() {
        let cases = [
            (
                CatalogInstallError::SessionActive,
                "mods cannot be installed while a game session is active",
            ),
            (
                CatalogInstallError::EntryNotFound { id: "a".into() },
                "no catalog entry with id \"a\"",
            ),
            (
                CatalogInstallError::NotDownloadable { id: "a".into() },
                "catalog entry \"a\" has no direct download",
            ),
            (
                CatalogInstallError::InstallInProgress { id: "a".into() },
                "catalog entry \"a\" is already being installed",
            ),
            (
                CatalogInstallError::Network("x".into()),
                "download failed: x",
            ),
            (
                CatalogInstallError::SizeMismatch {
                    expected: 1,
                    actual: 2,
                },
                "downloaded size 2 does not match the catalog size 1",
            ),
            (
                CatalogInstallError::HashMismatch,
                "the downloaded file does not match the catalog SHA-256",
            ),
            (CatalogInstallError::Cancelled, "install cancelled"),
            (
                CatalogInstallError::Storage("d".into()),
                "install storage error: d",
            ),
            (
                CatalogInstallError::Catalog(CatalogError::UnsupportedSchema { version: 9 }),
                "unsupported catalog schema version 9",
            ),
            (
                CatalogInstallError::Library(LibraryError::NoPakFound),
                "no .pak file found in the archive",
            ),
        ];
        for (error, text) in cases {
            assert_eq!(error.to_string(), text);
        }
    }
}
