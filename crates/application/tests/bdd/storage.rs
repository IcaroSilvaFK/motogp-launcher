//! Steps for `tests/features/storage.feature` (issue #56), with in-memory
//! fakes of the storage ports. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, FolderOpener, ScannedEntry, StorageScanner, StorageService};
use motogp_domain::{StorageCategory, StorageUsage};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FakeScanner {
    entries: Vec<ScannedEntry>,
    free: Option<u64>,
    scan_fails: bool,
}

impl StorageScanner for FakeScanner {
    fn scan(&self, root: &Path) -> Result<Vec<ScannedEntry>, AppError> {
        if self.scan_fails {
            return Err(AppError::Read {
                location: root.display().to_string(),
                detail: "access denied".into(),
            });
        }
        Ok(self.entries.clone())
    }
    fn free_space(&self, root: &Path) -> Result<u64, AppError> {
        self.free.ok_or_else(|| AppError::Read {
            location: root.display().to_string(),
            detail: "unknown".into(),
        })
    }
}

#[derive(Clone, Default)]
struct RecordingOpener(Arc<Mutex<Vec<PathBuf>>>);

impl FolderOpener for RecordingOpener {
    fn open_folder(&self, path: &Path) -> Result<(), AppError> {
        self.0.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[derive(Debug, Default, World)]
struct StorageWorld {
    root: PathBuf,
    entries: Vec<ScannedEntry>,
    free: Option<u64>,
    scan_fails: bool,
    opened: Arc<Mutex<Vec<PathBuf>>>,
    result: Option<Result<StorageUsage, AppError>>,
}

impl StorageWorld {
    fn service(&self) -> StorageService<FakeScanner, RecordingOpener> {
        let scanner = FakeScanner {
            entries: self.entries.clone(),
            free: self.free,
            scan_fails: self.scan_fails,
        };
        StorageService::new(
            self.root.clone(),
            scanner,
            RecordingOpener(self.opened.clone()),
        )
    }

    fn usage(&self) -> &StorageUsage {
        self.result
            .as_ref()
            .expect("usage not requested")
            .as_ref()
            .expect("usage failed")
    }
}

fn default_root() -> PathBuf {
    // Non-ASCII on purpose: Windows user folders often contain accents.
    PathBuf::from("C:/Users/Usuário Padrão/AppData/Roaming/motogp-launcher")
}

fn is_dir(name: &str) -> bool {
    name == "library"
}

#[given(
    expr = "the data folder {string} holds {string} with {int} bytes, {string} with {int} bytes, {string} with {int} bytes and {string} with {int} bytes"
)]
#[allow(clippy::too_many_arguments)]
fn holds_four(
    w: &mut StorageWorld,
    root: String,
    n1: String,
    b1: u64,
    n2: String,
    b2: u64,
    n3: String,
    b3: u64,
    n4: String,
    b4: u64,
) {
    w.root = PathBuf::from(root);
    w.entries = [(n1, b1), (n2, b2), (n3, b3), (n4, b4)]
        .into_iter()
        .map(|(name, bytes)| ScannedEntry {
            is_dir: is_dir(&name),
            name,
            bytes,
        })
        .collect();
    w.free = Some(0);
}

#[given(expr = "the drive has {int} bytes free")]
fn drive_free(w: &mut StorageWorld, bytes: u64) {
    w.free = Some(bytes);
}

#[given("the data folder holds nothing")]
fn holds_nothing(w: &mut StorageWorld) {
    w.root = default_root();
    w.entries.clear();
    w.free = Some(1);
}

#[given("the free space of the drive cannot be read")]
fn free_unreadable(w: &mut StorageWorld) {
    w.root = default_root();
    w.free = None;
}

#[given("the data folder cannot be scanned")]
fn scan_fails(w: &mut StorageWorld) {
    w.root = default_root();
    w.scan_fails = true;
}

#[when("the storage usage is requested")]
fn request_usage(w: &mut StorageWorld) {
    w.result = Some(w.service().usage());
}

#[when("the user opens the data folder")]
fn open_folder(w: &mut StorageWorld) {
    w.root = default_root();
    w.service().open_data_folder().expect("open folder");
}

#[then(expr = "mods use {int} bytes, launcher data {int} bytes and other {int} bytes")]
fn per_category(w: &mut StorageWorld, mods: u64, data: u64, other: u64) {
    let usage = w.usage();
    assert_eq!(usage.bytes_of(StorageCategory::Mods), mods);
    assert_eq!(usage.bytes_of(StorageCategory::LauncherData), data);
    assert_eq!(usage.bytes_of(StorageCategory::Other), other);
}

#[then(expr = "the total used is {int} bytes")]
fn total_used(w: &mut StorageWorld, bytes: u64) {
    assert_eq!(w.usage().total_bytes(), bytes);
}

#[then(expr = "the free space is {int} bytes")]
fn free_is(w: &mut StorageWorld, bytes: u64) {
    assert_eq!(w.usage().free_bytes, Some(bytes));
}

#[then("the data folder is reported with its accented path")]
fn accented_root(w: &mut StorageWorld) {
    let root = w.usage().root.to_string_lossy().into_owned();
    assert!(root.contains("Usuário Padrão"), "got: {root}");
}

#[then(expr = "every category uses {int} bytes and the total used is {int} bytes")]
fn all_zero(w: &mut StorageWorld, each: u64, total: u64) {
    let usage = w.usage();
    assert_eq!(usage.categories.len(), 3);
    assert!(usage.categories.iter().all(|(_, bytes)| *bytes == each));
    assert_eq!(usage.total_bytes(), total);
}

#[then("the usage is returned with the free space unknown")]
fn free_unknown(w: &mut StorageWorld) {
    assert_eq!(w.usage().free_bytes, None);
}

#[then("a read error naming the data folder is returned")]
fn read_error(w: &mut StorageWorld) {
    let error = w
        .result
        .as_ref()
        .expect("usage not requested")
        .as_ref()
        .expect_err("expected an error");
    assert!(matches!(error, AppError::Read { .. }), "got: {error:?}");
    assert!(error.location().contains("Usuário Padrão"), "got: {error}");
}

#[then("the folder opener receives exactly the data folder path")]
fn opener_received(w: &mut StorageWorld) {
    assert_eq!(*w.opened.lock().unwrap(), vec![default_root()]);
}

/// Runs only `storage.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = StorageWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("storage.feature"))
        .await;
    !writer.execution_has_failed()
}
