//! Steps for `tests/features/storage-classification.feature` (issue #56). Pure rules only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, World};
use motogp_domain::{classify_entry, StorageCategory, StorageUsage};
use std::path::PathBuf;

#[derive(Debug, Default, World)]
struct StorageWorld {
    entries: Vec<(String, bool)>,
    usage: Option<StorageUsage>,
}

fn label(category: StorageCategory) -> &'static str {
    match category {
        StorageCategory::Mods => "mods",
        StorageCategory::LauncherData => "launcher data",
        StorageCategory::Other => "other",
    }
}

#[given(expr = "an entry {string} that is a directory")]
fn directory_entry(w: &mut StorageWorld, name: String) {
    w.entries = vec![(name, true)];
}

#[given(expr = "the entries {string}, {string}, {string} and {string} that are files")]
fn four_files(w: &mut StorageWorld, a: String, b: String, c: String, d: String) {
    w.entries = [a, b, c, d].into_iter().map(|n| (n, false)).collect();
}

#[given(expr = "an entry {string} that is a file and an entry {string} that is a file")]
fn two_files(w: &mut StorageWorld, a: String, b: String) {
    w.entries = vec![(a, false), (b, false)];
}

#[given(expr = "a usage with mods {int} bytes, launcher data {int} bytes and other {int} bytes")]
fn a_usage(w: &mut StorageWorld, mods: u64, data: u64, other: u64) {
    w.usage = Some(StorageUsage::from_entries(
        PathBuf::from("root"),
        [
            (StorageCategory::Mods, mods),
            (StorageCategory::LauncherData, data),
            (StorageCategory::Other, other),
        ],
        None,
    ));
}

#[then(expr = "it is classified as {string}")]
fn classified_as(w: &mut StorageWorld, expected: String) {
    each_classified_as(w, expected);
}

#[then(expr = "each one is classified as {string}")]
fn each_classified_as(w: &mut StorageWorld, expected: String) {
    assert!(!w.entries.is_empty());
    for (name, is_dir) in &w.entries {
        assert_eq!(label(classify_entry(name, *is_dir)), expected, "{name}");
    }
}

#[then(expr = "the total is {int} bytes")]
fn total_is(w: &mut StorageWorld, expected: u64) {
    assert_eq!(w.usage.as_ref().expect("usage").total_bytes(), expected);
}

#[then(expr = "the categories are listed as mods, launcher data, other")]
fn listed_in_order(w: &mut StorageWorld) {
    let order: Vec<&str> = w
        .usage
        .as_ref()
        .expect("usage")
        .categories
        .iter()
        .map(|(c, _)| label(*c))
        .collect();
    assert_eq!(order, ["mods", "launcher data", "other"]);
}

/// Runs only `storage-classification.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = StorageWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("storage-classification.feature"))
        .await;
    !writer.execution_has_failed()
}
