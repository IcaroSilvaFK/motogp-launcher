//! Steps for `tests/features/mod-grouping.feature` (issue #3). Pure rules only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{display_name, group_pak_files, GroupingError, ModGroup};

#[derive(Debug, Default, World)]
struct GroupingWorld {
    names: Vec<String>,
    base: String,
    result: Option<Result<Vec<ModGroup>, GroupingError>>,
    display: Option<String>,
}

fn split_list(list: &str) -> Vec<String> {
    list.split(',').map(|s| s.trim().to_string()).collect()
}

#[given(expr = "a group containing only {string}")]
fn group_containing_only(w: &mut GroupingWorld, name: String) {
    w.names = vec![name];
}

#[given(expr = "the files {string}")]
fn the_files(w: &mut GroupingWorld, list: String) {
    w.names = split_list(&list);
}

#[when("I validate the grouping")]
fn validate(w: &mut GroupingWorld) {
    w.result = Some(group_pak_files(&w.names));
}

#[then(expr = "I receive an error containing {string}")]
fn error_containing(w: &mut GroupingWorld, text: String) {
    let error = w
        .result
        .clone()
        .expect("grouping not run")
        .expect_err("expected an error");
    assert!(error.to_string().contains(&text), "got: {error}");
}

#[then(expr = "I receive one group named {string} with the files {string}")]
fn one_group(w: &mut GroupingWorld, name: String, list: String) {
    let groups = w
        .result
        .clone()
        .expect("grouping not run")
        .expect("expected groups");
    assert_eq!(groups.len(), 1, "got: {groups:?}");
    assert_eq!(groups[0].display_name(), name);
    let mut actual = groups[0].files.clone();
    let mut expected = split_list(&list);
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[given(expr = "the base name {string}")]
fn base_name_given(w: &mut GroupingWorld, base: String) {
    w.base = base;
}

#[when("I ask for its display name")]
fn ask_display(w: &mut GroupingWorld) {
    w.display = Some(display_name(&w.base));
}

#[then(expr = "the display name is {string}")]
fn display_is(w: &mut GroupingWorld, expected: String) {
    assert_eq!(w.display.as_deref(), Some(expected.as_str()));
}

/// Runs only `mod-grouping.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = GroupingWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("mod-grouping.feature"))
        .await;
    !writer.execution_has_failed()
}
