//! BDD dispatcher (cucumber, `harness = false`).
//!
//! Each feature owns one step module in `tests/bdd/<feature>.rs` with its own
//! `World` and a `pub async fn run() -> bool` that runs only its `.feature`
//! file. To add a feature: add one `#[path]` module and one `FEATURES` entry.
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

#[path = "bdd/commands_and_recovery.rs"]
mod commands_and_recovery;
#[path = "bdd/deploy.rs"]
mod deploy;
#[path = "bdd/game_detection.rs"]
mod game_detection;
#[path = "bdd/library.rs"]
mod library;
#[path = "bdd/profiles.rs"]
mod profiles;

#[path = "bdd/session_tracking.rs"]
mod session_tracking;
#[path = "bdd/stale_manifest.rs"]
mod stale_manifest;

#[path = "support/config_fakes.rs"]
pub mod config_fakes;
#[path = "support/deploy_fakes.rs"]
pub mod deploy_fakes;
#[path = "support/fakes.rs"]
pub mod library_fakes;
#[path = "support/play_fakes.rs"]
pub mod play_fakes;

type Runner = fn() -> Pin<Box<dyn Future<Output = bool>>>;

const FEATURES: &[(&str, Runner)] = &[
    ("game-detection", || Box::pin(game_detection::run())),
    ("profiles", || Box::pin(profiles::run())),
    ("library", || Box::pin(library::run())),
    ("session-tracking", || Box::pin(session_tracking::run())),
    ("deploy", || Box::pin(deploy::run())),
    ("commands-and-recovery", || {
        Box::pin(commands_and_recovery::run())
    }),
    ("stale-manifest", || Box::pin(stale_manifest::run())),
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
