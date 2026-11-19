//! Steps for `tests/features/processes.feature` (issue #7). Controlled paths only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_infrastructure::is_inside_root;
use std::path::PathBuf;

#[derive(Debug, Default, World)]
struct ProcessWorld {
    root: PathBuf,
    executables: Vec<PathBuf>,
    verdicts: Vec<bool>,
}

#[given(expr = "a game root {string}")]
fn game_root(w: &mut ProcessWorld, root: String) {
    w.root = PathBuf::from(root);
}

#[given(expr = "executables at {string} and {string}")]
fn executables(w: &mut ProcessWorld, first: String, second: String) {
    w.executables = vec![PathBuf::from(first), PathBuf::from(second)];
}

#[when("I classify the processes")]
fn classify(w: &mut ProcessWorld) {
    w.verdicts = w
        .executables
        .iter()
        .map(|exe| is_inside_root(exe, &w.root))
        .collect();
}

#[then("only the first is considered a game process")]
fn only_first(w: &mut ProcessWorld) {
    assert_eq!(w.verdicts, vec![true, false]);
}

/// Runs only `processes.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ProcessWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("processes.feature"))
        .await;
    !writer.execution_has_failed()
}
