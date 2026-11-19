//! Steps for `tests/features/deploy-names.feature` (issue #6). Pure rules only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{deployed_name, is_safe_file_name};

#[derive(Debug, Default, World)]
struct NamesWorld {
    /// (position, mod id, original file name)
    inputs: Vec<(usize, String, String)>,
    original: String,
    unsafe_candidate: String,
    result: Vec<String>,
    safe: Option<bool>,
}

#[given(expr = "the mods {string} and {string} in that order, with the IDs {string} and {string}")]
fn two_mods(w: &mut NamesWorld, a: String, b: String, id_a: String, id_b: String) {
    w.inputs = vec![
        (0, id_a, format!("{a}.pak")),
        (1, id_b, format!("{b}.pak")),
    ];
}

#[given(expr = "an original file {string}")]
fn original_file(w: &mut NamesWorld, name: String) {
    w.original = name;
}

#[given(expr = "the mod {string} at position {int} with the files {string}")]
fn mod_at(w: &mut NamesWorld, _name: String, position: usize, list: String) {
    w.inputs = list
        .split(',')
        .map(|f| (position, "aaaaaaaaaaaa".to_string(), f.trim().to_string()))
        .collect();
}

#[given(expr = "a file name {string}")]
fn file_name(w: &mut NamesWorld, name: String) {
    w.unsafe_candidate = name;
}

#[when("I compute the destination of their files")]
fn compute_all(w: &mut NamesWorld) {
    w.result = w
        .inputs
        .iter()
        .map(|(i, id, f)| deployed_name(*i, id, f))
        .collect();
}

#[when("I compute its destination")]
fn compute_one(w: &mut NamesWorld) {
    w.result = vec![deployed_name(0, "aaaaaaaaaaaa", &w.original)];
}

#[when("I check whether the name is safe")]
fn check_safe(w: &mut NamesWorld) {
    w.safe = Some(is_safe_file_name(&w.unsafe_candidate));
}

#[then(expr = "the destinations are {string} and {string}")]
fn destinations(w: &mut NamesWorld, a: String, b: String) {
    assert_eq!(w.result, vec![a, b]);
}

#[then(expr = "the name ends in {string}")]
fn ends_in(w: &mut NamesWorld, suffix: String) {
    assert!(w.result[0].ends_with(&suffix), "got {:?}", w.result);
}

#[then(expr = "all destinations start with {string}")]
fn start_with(w: &mut NamesWorld, prefix: String) {
    assert_eq!(w.result.len(), 3);
    assert!(w.result.iter().all(|n| n.starts_with(&prefix)));
}

#[then("the name is rejected")]
fn rejected(w: &mut NamesWorld) {
    assert_eq!(w.safe, Some(false));
}

/// Runs only `deploy-names.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = NamesWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("deploy-names.feature"))
        .await;
    !writer.execution_has_failed()
}
