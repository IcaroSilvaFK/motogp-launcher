//! Ports of the catalog download (issue #74): the HTTP downloader and the
//! staging area. Pure std, no IO here.
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cheap shared cancellation flag: clones observe the same state.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What to download. `max_bytes` is a hard cap: the declared catalog size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub url: String,
    pub max_bytes: u64,
}

/// Bytes received so far and the expected total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: u64,
}

/// Why a download did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// The server could not be reached or answered with an error.
    Network(String),
    /// The body is larger than the allowed `limit` in bytes.
    TooLarge { limit: u64 },
    /// The caller cancelled the download.
    Cancelled,
    /// The destination could not be written.
    Storage(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::Network(detail) => write!(f, "network error: {detail}"),
            DownloadError::TooLarge { limit } => write!(f, "download larger than {limit} bytes"),
            DownloadError::Cancelled => write!(f, "download cancelled"),
            DownloadError::Storage(detail) => write!(f, "could not write the download: {detail}"),
        }
    }
}

impl std::error::Error for DownloadError {}

/// Port: streams a URL to a file (HTTPS in production, fakes in tests).
pub trait ModDownloader: Send + Sync {
    /// Writes the body to `dest`, reporting progress and checking `cancel`
    /// between chunks. On failure `dest` may hold a partial file.
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelFlag,
    ) -> Result<(), DownloadError>;
}

/// A shared downloader is a downloader: lets the interface inject `Arc<dyn ModDownloader>`.
impl<T: ModDownloader + ?Sized> ModDownloader for Arc<T> {
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        (**self).download(request, dest, progress, cancel)
    }
}

/// Port: the scratch area `<data dir>/downloads/<entry id>/` of one install.
pub trait DownloadStaging: Send + Sync {
    /// A fresh, empty directory for the entry (any previous one is replaced).
    fn create(&self, entry_id: &str) -> Result<PathBuf, DownloadError>;
    /// Size in bytes of a staged file.
    fn file_size(&self, file: &Path) -> Result<u64, DownloadError>;
    /// Renames a staged file.
    fn rename(&self, from: &Path, to: &Path) -> Result<(), DownloadError>;
    /// Removes the entry's directory and everything in it. Idempotent.
    fn remove(&self, entry_id: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_flag_clones_share_state() {
        let flag = CancelFlag::new();
        let other = flag.clone();
        assert!(!other.is_cancelled());
        flag.cancel();
        assert!(other.is_cancelled());
    }

    #[test]
    fn download_errors_have_english_messages() {
        assert_eq!(
            DownloadError::Network("boom".into()).to_string(),
            "network error: boom"
        );
        assert_eq!(
            DownloadError::TooLarge { limit: 5 }.to_string(),
            "download larger than 5 bytes"
        );
        assert_eq!(DownloadError::Cancelled.to_string(), "download cancelled");
        assert_eq!(
            DownloadError::Storage("disk".into()).to_string(),
            "could not write the download: disk"
        );
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;

    struct Writes;

    impl ModDownloader for Writes {
        fn download(
            &self,
            request: &DownloadRequest,
            _dest: &Path,
            progress: &mut dyn FnMut(DownloadProgress),
            _cancel: &CancelFlag,
        ) -> Result<(), DownloadError> {
            progress(DownloadProgress {
                received: request.max_bytes,
                total: request.max_bytes,
            });
            Ok(())
        }
    }

    #[test]
    fn a_shared_dyn_downloader_forwards_to_the_inner_downloader() {
        let shared: Arc<dyn ModDownloader> = Arc::new(Writes);
        let mut seen = Vec::new();
        let request = DownloadRequest {
            url: "https://example.com/a.zip".into(),
            max_bytes: 7,
        };
        let result = shared.download(
            &request,
            Path::new("a"),
            &mut |p| seen.push(p),
            &CancelFlag::new(),
        );
        assert_eq!(result, Ok(()));
        assert_eq!(
            seen,
            vec![DownloadProgress {
                received: 7,
                total: 7
            }]
        );
    }
}
