//! BDD dispatcher (cucumber, `harness = false`).
//!
//! Each feature owns one step module in `tests/bdd/<feature>.rs` with its own
//! `World` and a `pub async fn run() -> bool` that runs only its `.feature`
//! file. To add a feature: add one `#[path]` module and one `FEATURES` entry.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

#[path = "bdd/game_detection.rs"]
mod game_detection;
#[path = "bdd/deploy.rs"]
mod deploy;
#[path = "bdd/library.rs"]
mod library;
#[path = "bdd/profiles.rs"]
mod profiles;

#[path = "bdd/session_tracking.rs"]
mod session_tracking;

type Runner = fn() -> Pin<Box<dyn Future<Output = bool>>>;

const FEATURES: &[(&str, Runner)] = &[
    ("game-detection", || Box::pin(game_detection::run())),
    ("profiles", || Box::pin(profiles::run())),
    ("library", || Box::pin(library::run())),
    ("session-tracking", || Box::pin(session_tracking::run())),
    ("deploy", || Box::pin(deploy::run())),
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
