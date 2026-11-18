//! BDD dispatcher (cucumber, `harness = false`).
//!
//! Each feature owns one step module in `tests/bdd/<feature>.rs` with its own
//! `World` and a `pub async fn run() -> bool` that runs only its `.feature`
//! file. To add a feature: add one `#[path]` module and one `FEATURES` entry.
//! No feature is registered yet; later issues add theirs here.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

type Runner = fn() -> Pin<Box<dyn Future<Output = bool>>>;

const FEATURES: &[(&str, Runner)] = &[];

/// Absolute path of a `.feature` file in `tests/features/`.
#[allow(dead_code)] // used as soon as the first feature is registered
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
