//! Steps for `tests/features/library.feature` (issue #3), using in-memory
//! fakes of the ports. Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::LibraryError;
use motogp_domain::ModEntry;
use std::path::PathBuf;

use crate::library_fakes as fakes;
use fakes::{
    id_of, library_files_of, new_state, put_archive, put_file, service, staging_leftovers, Shared,
};

const PAK: &[u8] = b"AAAAAA-pak";

#[derive(Debug, World)]
#[world(init = Self::new)]
struct LibraryWorld {
    state: Shared,
    source: PathBuf,
    result: Option<Result<Vec<ModEntry>, LibraryError>>,
    first: Option<Vec<ModEntry>>,
    removed_id: String,
}

impl LibraryWorld {
    fn new() -> Self {
        Self {
            state: new_state(),
            source: PathBuf::new(),
            result: None,
            first: None,
            removed_id: String::new(),
        }
    }

    fn import(&mut self) {
        self.result = Some(service(&self.state).import(&self.source));
    }

    fn entries(&self) -> Vec<ModEntry> {
        self.result
            .clone()
            .expect("import not run")
            .expect("import failed")
    }
}

// ---------- Scenario: Import a loose pak ----------

#[given(expr = "a file {string}")]
fn a_file(w: &mut LibraryWorld, name: String) {
    put_file(&w.state, &format!("src/{name}"), PAK);
    w.source = PathBuf::from(format!("src/{name}"));
}

#[when("I import it")]
fn import_it(w: &mut LibraryWorld) {
    w.import();
}

#[then(expr = "I receive one entry with the file {string}")]
fn one_entry_with_file(w: &mut LibraryWorld, name: String) {
    let entries = w.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].files, vec![name]);
}

#[then("its ID has 12 lowercase hexadecimal characters derived from the hash of the pak")]
fn id_derived(w: &mut LibraryWorld) {
    let id = &w.entries()[0].id;
    assert_eq!(id.len(), 12);
    assert!(id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
    assert_eq!(*id, id_of(PAK));
}

#[then("the index and library/<id>/A.pak contain the imported mod")]
fn index_and_folder(w: &mut LibraryWorld) {
    let entry = w.entries().remove(0);
    assert_eq!(w.state.borrow().index, vec![entry.clone()]);
    assert_eq!(library_files_of(&w.state, &entry.id), vec!["A.pak"]);
}

// ---------- Scenario: Pick up only the siblings ----------

#[given(expr = "{string}, {string}, {string} and {string} in the same folder")]
fn siblings(w: &mut LibraryWorld, a: String, b: String, c: String, d: String) {
    for (name, content) in [
        (a, PAK),
        (b, b"utoc".as_slice()),
        (c, b"ucas!".as_slice()),
        (d, b"BBBBBB-pak".as_slice()),
    ] {
        put_file(&w.state, &format!("src/{name}"), content);
    }
}

#[when(expr = "I import {string}")]
fn import_named(w: &mut LibraryWorld, name: String) {
    w.source = PathBuf::from(format!("src/{name}"));
    w.import();
}

#[then(expr = "the mod is named {string} and has the three files of A_P")]
fn named_with_three(w: &mut LibraryWorld, name: String) {
    let entries = w.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, name);
    let mut files = entries[0].files.clone();
    files.sort();
    assert_eq!(files, vec!["A_P.pak", "A_P.ucas", "A_P.utoc"]);
}

#[then("B is not imported")]
fn b_not_imported(w: &mut LibraryWorld) {
    assert!(w.state.borrow().index.iter().all(|e| e.name != "B"));
    assert_eq!(w.state.borrow().index.len(), 1);
}

// ---------- Scenario: ZIP subfolders ----------

#[given(expr = "a ZIP containing {string} and {string}")]
fn zip_with_two(w: &mut LibraryWorld, a: String, b: String) {
    // The reader port returns flattened names, as the real adapter does.
    let flat = |n: &str| n.rsplit('/').next().unwrap().to_string();
    let (fa, fb) = (flat(&a), flat(&b));
    put_archive(
        &w.state,
        "mods.zip",
        &[(fa.as_str(), PAK), (fb.as_str(), b"BBBBBB-pak")],
    );
    w.source = PathBuf::from("mods.zip");
}

#[then("I receive two entries with flattened file names")]
fn two_entries(w: &mut LibraryWorld) {
    let entries = w.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].files, vec!["A.pak"]);
    assert_eq!(entries[1].files, vec!["B.pak"]);
}

// ---------- Scenario: ZIP without pak ----------

#[given(expr = "a ZIP containing only {string}")]
fn zip_only(w: &mut LibraryWorld, name: String) {
    put_archive(&w.state, "mods.zip", &[(name.as_str(), b"hi")]);
    w.source = PathBuf::from("mods.zip");
}

#[then(expr = "I receive an error containing {string}")]
fn error_containing(w: &mut LibraryWorld, text: String) {
    let error = w
        .result
        .clone()
        .expect("import not run")
        .expect_err("expected an error");
    assert!(error.to_string().contains(&text), "got: {error}");
}

// ---------- Scenario: Reimport ----------

#[given("a pak that was already imported")]
fn already_imported(w: &mut LibraryWorld) {
    put_file(&w.state, "src/A.pak", PAK);
    w.source = PathBuf::from("src/A.pak");
    w.first = Some(service(&w.state).import(&w.source).unwrap());
}

#[when("I import the same content again")]
fn import_again(w: &mut LibraryWorld) {
    w.import();
}

#[then("I receive the existing entry")]
fn existing_entry(w: &mut LibraryWorld) {
    assert_eq!(Some(w.entries()), w.first.clone());
}

#[then("the library still has one entry for that ID")]
fn one_entry_for_id(w: &mut LibraryWorld) {
    let id = w.first.clone().unwrap()[0].id.clone();
    let count = w.state.borrow().index.iter().filter(|e| e.id == id).count();
    assert_eq!(count, 1);
}

// ---------- Scenario: Remove ----------

#[given("an imported mod")]
fn imported_mod(w: &mut LibraryWorld) {
    already_imported(w);
}

#[when("I remove its ID")]
fn remove_id(w: &mut LibraryWorld) {
    w.removed_id = w.first.clone().unwrap()[0].id.clone();
    service(&w.state).remove(&w.removed_id).unwrap();
}

#[then("its folder and index entry no longer exist")]
fn gone(w: &mut LibraryWorld) {
    assert!(w.state.borrow().index.is_empty());
    assert!(library_files_of(&w.state, &w.removed_id).is_empty());
}

// ---------- Scenario: Unsupported format / failed import ----------

#[when("I try to import it")]
fn try_import(w: &mut LibraryWorld) {
    w.import();
}

#[given(expr = "a file {string} that cannot be copied")]
fn uncopyable(w: &mut LibraryWorld, name: String) {
    a_file(w, name);
    w.state.borrow_mut().fail_copy = true;
}

#[then("I receive an error")]
fn receive_error(w: &mut LibraryWorld) {
    assert!(matches!(w.result, Some(Err(_))), "got {:?}", w.result);
}

#[then("the index stays empty")]
fn index_empty(w: &mut LibraryWorld) {
    assert!(w.state.borrow().index.is_empty());
}

#[then("no temporary files remain")]
fn no_temp(w: &mut LibraryWorld) {
    assert!(staging_leftovers(&w.state).is_empty());
}

/// Runs only `library.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = LibraryWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("library.feature"))
        .await;
    !writer.execution_has_failed()
}
