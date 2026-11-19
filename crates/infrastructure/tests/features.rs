//! BDD dispatcher (cucumber, `harness = false`).
//!
//! Each feature owns one step module in `tests/bdd/<feature>.rs` with its own
//! `World` and a `pub async fn run() -> bool` that runs only its `.feature`
//! file. To add a feature: add one `#[path]` module and one `FEATURES` entry.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

#[path = "bdd/steam.rs"]
mod steam;

#[path = "bdd/file_import.rs"]
mod file_import;
#[path = "bdd/processes.rs"]
mod processes;

#[path = "bdd/persistence.rs"]
mod persistence;
#[path = "bdd/profile_persistence.rs"]
mod profile_persistence;

type Runner = fn() -> Pin<Box<dyn Future<Output = bool>>>;

const FEATURES: &[(&str, Runner)] = &[
    ("persistence", || Box::pin(persistence::run())),
    ("profile-persistence", || {
        Box::pin(profile_persistence::run())
    }),
    ("processes", || Box::pin(processes::run())),
    ("steam", || Box::pin(steam::run())),
    ("file_import", || Box::pin(file_import::run())),
];

/// Absolute path of a `.feature` file in `tests/features/`.
pub fn feature(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/features")
        .join(file)
}

fn main() {
    let failed: Vec<&str> = FEATURES
        .iter()
        .filter(|(_, run)| !futures::executor::block_on(run()))
        .map(|(name, _)| *name)
        .collect();
    if !failed.is_empty() {
        eprintln!("BDD features failed: {failed:?}");
        std::process::exit(1);
    }
}
