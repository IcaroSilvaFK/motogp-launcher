//! Steps for `tests/features/load-order.feature` (issue #55). Pure rules only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{detect_conflicts, FileConflict, ModEntry, Profile};

#[derive(Debug, Default, World)]
struct LoadOrderWorld {
    library: Vec<ModEntry>,
    conflicts: Vec<FileConflict>,
    profile: Option<Profile>,
}

fn entry(id: &str, file: &str) -> ModEntry {
    ModEntry {
        id: id.to_string(),
        name: id.to_string(),
        files: vec![file.to_string()],
        size_bytes: 0,
    }
}

#[given(expr = "mod {string} ships {string} and mod {string} ships {string}")]
fn two_mods(w: &mut LoadOrderWorld, a: String, file_a: String, b: String, file_b: String) {
    w.library = vec![entry(&a, &file_a), entry(&b, &file_b)];
}

#[when(expr = "conflicts are detected for the order {string}, {string}")]
fn order_two(w: &mut LoadOrderWorld, a: String, b: String) {
    w.conflicts = detect_conflicts(&[a, b], &w.library);
}

#[when(expr = "conflicts are detected for the order {string}, {string}, {string}")]
fn order_three(w: &mut LoadOrderWorld, a: String, b: String, c: String) {
    w.conflicts = detect_conflicts(&[a, b, c], &w.library);
}

#[then(expr = "there is one conflict on {string} involving {string} then {string}")]
fn one_conflict(w: &mut LoadOrderWorld, path: String, first: String, second: String) {
    assert_eq!(w.conflicts.len(), 1);
    assert_eq!(w.conflicts[0].path, path);
    assert_eq!(w.conflicts[0].mod_ids, [first, second]);
}

#[then(expr = "the winner is {string}")]
fn winner(w: &mut LoadOrderWorld, id: String) {
    assert_eq!(w.conflicts[0].winner(), id);
}

#[then("no conflict is detected")]
fn none(w: &mut LoadOrderWorld) {
    assert!(w.conflicts.is_empty());
}

#[when(expr = "the load order is set to {string}, {string}, {string}, {string}")]
fn set_order(w: &mut LoadOrderWorld, a: String, b: String, c: String, d: String) {
    let mut profile = Profile::new("p1", "x").unwrap();
    profile.set_load_order(vec![a, b, c, d]);
    w.profile = Some(profile);
}

#[then(expr = "the profile order is {string}, {string}, {string}")]
fn profile_order(w: &mut LoadOrderWorld, a: String, b: String, c: String) {
    assert_eq!(w.profile.as_ref().unwrap().mod_ids, [a, b, c]);
}

/// Runs only `load-order.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = LoadOrderWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("load-order.feature"))
        .await;
    !writer.execution_has_failed()
}
