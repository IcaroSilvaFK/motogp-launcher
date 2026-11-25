//! In-memory fakes of the catalog download ports (issue #74). No network, no disk.
//! The including crate root must expose `catalog_fakes` and `library_fakes`.
#![allow(dead_code)]
use crate::catalog_fakes::{draft, fetched, FakeCache, FakeClock, FakeSource, NOW_MS};
use crate::library_fakes::fake_hash;
use motogp_application::{
    CancelFlag, CatalogInstaller, CatalogService, ContentHasher, DownloadError, DownloadProgress,
    DownloadRequest, DownloadStaging, LibraryError, ModDownloader, ProfileService, RawEntry,
    SessionActivity,
};
use motogp_domain::CatalogDownloadDraft;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The staging "disk": files by path plus the directories that exist.
#[derive(Default)]
pub struct Disk {
    pub files: BTreeMap<PathBuf, Vec<u8>>,
    pub dirs: Vec<PathBuf>,
}

pub type SharedDisk = Arc<Mutex<Disk>>;

impl Disk {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.dirs.is_empty()
    }
}

#[derive(Clone)]
pub struct FakeStaging(pub SharedDisk);

impl DownloadStaging for FakeStaging {
    fn create(&self, entry_id: &str) -> Result<PathBuf, DownloadError> {
        let dir = PathBuf::from("downloads").join(entry_id);
        let mut disk = self.0.lock().unwrap();
        if !disk.dirs.contains(&dir) {
            disk.dirs.push(dir.clone());
        }
        Ok(dir)
    }
    fn file_size(&self, file: &Path) -> Result<u64, DownloadError> {
        self.0
            .lock()
            .unwrap()
            .files
            .get(file)
            .map(|c| c.len() as u64)
            .ok_or_else(|| DownloadError::Storage("missing file".into()))
    }
    fn rename(&self, from: &Path, to: &Path) -> Result<(), DownloadError> {
        let mut disk = self.0.lock().unwrap();
        let content = disk
            .files
            .remove(from)
            .ok_or_else(|| DownloadError::Storage("missing file".into()))?;
        disk.files.insert(to.to_path_buf(), content);
        Ok(())
    }
    fn remove(&self, entry_id: &str) {
        let dir = PathBuf::from("downloads").join(entry_id);
        let mut disk = self.0.lock().unwrap();
        disk.files.retain(|path, _| !path.starts_with(&dir));
        disk.dirs.retain(|d| *d != dir);
    }
}

#[derive(Clone)]
pub struct FakeHasher(pub SharedDisk);

impl ContentHasher for FakeHasher {
    fn sha256_hex(&self, file: &Path) -> Result<String, LibraryError> {
        self.0
            .lock()
            .unwrap()
            .files
            .get(file)
            .map(|c| fake_hash(c))
            .ok_or(LibraryError::NoPakFound)
    }
}

/// How the fake downloader behaves.
#[derive(Clone, Default)]
pub enum Behavior {
    /// Writes `content` and reports one progress step.
    #[default]
    Serve,
    /// Fails with this error without writing anything.
    Fail(DownloadError),
    /// Writes half of the content, raises the cancel flag and reports `Cancelled`.
    CancelMidway,
}

#[derive(Default)]
struct DownloaderInner {
    calls: AtomicUsize,
    requests: Mutex<Vec<DownloadRequest>>,
    dests: Mutex<Vec<PathBuf>>,
    cancel_seen: AtomicBool,
}

#[derive(Clone)]
pub struct FakeDownloader {
    disk: SharedDisk,
    pub content: Arc<Mutex<Vec<u8>>>,
    pub behavior: Arc<Mutex<Behavior>>,
    inner: Arc<DownloaderInner>,
}

impl FakeDownloader {
    pub fn new(disk: &SharedDisk, content: &[u8]) -> Self {
        Self {
            disk: disk.clone(),
            content: Arc::new(Mutex::new(content.to_vec())),
            behavior: Arc::default(),
            inner: Arc::default(),
        }
    }
    pub fn behave(&self, behavior: Behavior) {
        *self.behavior.lock().unwrap() = behavior;
    }
    pub fn calls(&self) -> usize {
        self.inner.calls.load(Ordering::SeqCst)
    }
    pub fn requests(&self) -> Vec<DownloadRequest> {
        self.inner.requests.lock().unwrap().clone()
    }
    pub fn dests(&self) -> Vec<PathBuf> {
        self.inner.dests.lock().unwrap().clone()
    }
}

impl ModDownloader for FakeDownloader {
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        self.inner.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.requests.lock().unwrap().push(request.clone());
        self.inner.dests.lock().unwrap().push(dest.to_path_buf());
        self.inner
            .cancel_seen
            .store(cancel.is_cancelled(), Ordering::SeqCst);
        let content = self.content.lock().unwrap().clone();
        let behavior = self.behavior.lock().unwrap().clone();
        match behavior {
            Behavior::Fail(error) => Err(error),
            Behavior::CancelMidway => {
                let half = content[..content.len() / 2].to_vec();
                self.disk
                    .lock()
                    .unwrap()
                    .files
                    .insert(dest.to_path_buf(), half);
                cancel.cancel();
                Err(DownloadError::Cancelled)
            }
            Behavior::Serve => {
                let total = content.len() as u64;
                self.disk
                    .lock()
                    .unwrap()
                    .files
                    .insert(dest.to_path_buf(), content);
                progress(DownloadProgress {
                    received: total,
                    total: request.max_bytes,
                });
                Ok(())
            }
        }
    }
}

/// Session flag a test can flip.
#[derive(Clone, Default)]
pub struct FakeSession(pub Arc<AtomicBool>);

impl FakeSession {
    pub fn set_active(&self, active: bool) {
        self.0.store(active, Ordering::SeqCst);
    }
}

impl SessionActivity for FakeSession {
    fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A catalog draft with a download of `content` (or custom size/hash).
pub fn downloadable(id: &str, url: &str, size: u64, sha256: &str) -> RawEntry {
    let mut entry = draft(id);
    entry.download = Some(CatalogDownloadDraft {
        url: url.to_string(),
        size: Some(size),
        sha256: Some(sha256.to_string()),
    });
    RawEntry::Draft(entry)
}

pub type TestInstaller = CatalogInstaller<FakeDownloader, FakeStaging, FakeHasher>;
pub type TestCatalog = CatalogService<FakeSource, FakeCache, FakeClock>;

pub struct Rig {
    pub disk: SharedDisk,
    pub downloader: FakeDownloader,
    pub installer: TestInstaller,
    pub catalog: TestCatalog,
    pub session: FakeSession,
}

impl Rig {
    /// A rig whose catalog serves `entries` and whose "server" serves `content`.
    pub fn new(entries: Vec<RawEntry>, content: &[u8]) -> Self {
        let disk = SharedDisk::default();
        let downloader = FakeDownloader::new(&disk, content);
        let installer = CatalogInstaller::new(
            downloader.clone(),
            FakeStaging(disk.clone()),
            FakeHasher(disk.clone()),
        );
        let source = FakeSource::answering(vec![Ok(fetched(1, entries, None))]);
        let catalog = CatalogService::new(source, FakeCache::default(), FakeClock::at(NOW_MS));
        Self {
            disk,
            downloader,
            installer,
            catalog,
            session: FakeSession::default(),
        }
    }

    /// A rig with one entry "rebalanced-ai" that matches `content`.
    pub fn matching(content: &[u8]) -> Self {
        let entry = downloadable(
            "rebalanced-ai",
            "https://example.com/files/rebalanced.pak",
            content.len() as u64,
            &fake_hash(content),
        );
        Self::new(vec![entry], content)
    }

    pub fn profiles(
        &self,
    ) -> ProfileService<crate::config_fakes::FakeProfiles, crate::config_fakes::FixedClock> {
        ProfileService::new(
            crate::config_fakes::FakeProfiles::default(),
            crate::config_fakes::FixedClock(1_000),
        )
        .with_session(Arc::new(self.session.clone()))
    }

    pub fn disk_is_empty(&self) -> bool {
        self.disk.lock().unwrap().is_empty()
    }
}
