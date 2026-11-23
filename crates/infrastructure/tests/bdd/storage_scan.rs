//! Steps for `tests/features/storage-scan.feature` (issue #56), over temp dirs.
//! Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, ScannedEntry, StorageScanner};
use motogp_infrastructure::FsStorageScanner;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug, Default, World)]
struct ScanWorld {
    tmp: Option<TempDir>,
    root: PathBuf,
    /// False when the OS refused to create the link (the scenario then
    /// checks nothing and says so).
    link_created: bool,
    result: Option<Result<Vec<ScannedEntry>, AppError>>,
}

impl ScanWorld {
    fn entries(&self) -> &Vec<ScannedEntry> {
        self.result
            .as_ref()
            .expect("folder not scanned")
            .as_ref()
            .expect("scan failed")
    }

    fn find(&self, name: &str) -> &ScannedEntry {
        self.entries()
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("no entry {name}"))
    }
}

fn write(path: &Path, bytes: u64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes as usize]).unwrap();
}

/// Creates a directory link (junction on Windows, symlink elsewhere).
fn link_dir(link: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

#[given(
    expr = "a data folder under {string} with {string} of {int} bytes, {string} of {int} bytes and {string} of {int} bytes"
)]
#[allow(clippy::too_many_arguments)]
fn data_folder_with(
    w: &mut ScanWorld,
    parent: String,
    n1: String,
    b1: u64,
    n2: String,
    b2: u64,
    n3: String,
    b3: u64,
) {
    let tmp = TempDir::new().unwrap();
    w.root = tmp.path().join(parent).join("data");
    for (name, bytes) in [(n1, b1), (n2, b2), (n3, b3)] {
        write(&w.root.join(name), bytes);
    }
    w.tmp = Some(tmp);
}

#[given("the data folder does not exist yet")]
fn missing_folder(w: &mut ScanWorld) {
    let tmp = TempDir::new().unwrap();
    w.root = tmp.path().join("not-yet");
    w.tmp = Some(tmp);
}

#[given("the data folder contains a directory link to a folder with 1000 bytes")]
fn folder_with_link(w: &mut ScanWorld) {
    let tmp = TempDir::new().unwrap();
    write(&tmp.path().join("outside").join("big.bin"), 1000);
    w.root = tmp.path().join("data");
    fs::create_dir_all(&w.root).unwrap();
    w.link_created = link_dir(&w.root.join("link"), &tmp.path().join("outside"));
    if !w.link_created {
        println!("note: the OS refused to create a directory link; link check skipped");
    }
    w.tmp = Some(tmp);
}

#[when("the folder is scanned")]
fn scan(w: &mut ScanWorld) {
    w.result = Some(FsStorageScanner.scan(&w.root));
}

#[then(expr = "{string} is a directory of {int} bytes")]
fn directory_of(w: &mut ScanWorld, name: String, bytes: u64) {
    let entry = w.find(&name);
    assert!(entry.is_dir);
    assert_eq!(entry.bytes, bytes);
}

#[then(expr = "{string} is a file of {int} bytes")]
fn file_of(w: &mut ScanWorld, name: String, bytes: u64) {
    let entry = w.find(&name);
    assert!(!entry.is_dir);
    assert_eq!(entry.bytes, bytes);
}

#[then("the free space of the drive is greater than 0")]
fn free_space(w: &mut ScanWorld) {
    assert!(FsStorageScanner.free_space(&w.root).unwrap() > 0);
}

#[then("the scan returns no entries")]
fn no_entries(w: &mut ScanWorld) {
    assert!(w.entries().is_empty());
}

#[then("the bytes behind the link are not counted")]
fn link_not_counted(w: &mut ScanWorld) {
    if !w.link_created {
        return;
    }
    let total: u64 = w.entries().iter().map(|e| e.bytes).sum();
    assert_eq!(total, 0, "{:?}", w.entries());
}

/// Runs only `storage-scan.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ScanWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("storage-scan.feature"))
        .await;
    !writer.execution_has_failed()
}
