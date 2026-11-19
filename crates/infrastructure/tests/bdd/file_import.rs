//! Steps for `tests/features/file-import.feature` (issue #3): real fs, ZIP and
//! SHA-256 adapters in temporary directories. Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{LibraryError, ModRepository};
use motogp_domain::ModEntry;
use motogp_infrastructure::{mod_library, DataDir, JsonModRepository};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[path = "../support/zips.rs"]
mod zips;

#[derive(Debug, Default, World)]
struct ImportWorld {
    tmp: Option<TempDir>,
    source: PathBuf,
    result: Option<Result<Vec<ModEntry>, LibraryError>>,
    first: Option<Vec<ModEntry>>,
    removed_id: String,
    library_file_before: Vec<u8>,
}

impl ImportWorld {
    fn root(&mut self) -> PathBuf {
        self.tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
            .path()
            .join("Usuário Padrão")
    }

    fn src(&mut self) -> PathBuf {
        let dir = self.root().join("downloads");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn data(&mut self) -> DataDir {
        DataDir::new(self.root().join("data"))
    }

    fn import(&mut self) {
        let data = self.data();
        self.result = Some(mod_library(&data).import(&self.source));
    }

    fn entries(&self) -> Vec<ModEntry> {
        self.result
            .clone()
            .expect("import not run")
            .expect("import failed")
    }

    fn index(&mut self) -> Vec<ModEntry> {
        JsonModRepository::new(self.data().library_file())
            .list()
            .unwrap()
    }
}

// ---------- Scenario: Import a loose pak ----------

#[given(expr = "a file {string} with the content {string}")]
fn file_with_content(w: &mut ImportWorld, name: String, content: String) {
    let path = w.src().join(&name);
    fs::write(&path, content).unwrap();
    w.source = path;
}

#[when("I import it")]
fn import_it(w: &mut ImportWorld) {
    w.import();
}

#[then(expr = "I receive an entry with the file {string}")]
fn entry_with_file(w: &mut ImportWorld, name: String) {
    let entries = w.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].files, vec![name]);
}

#[then(expr = "its ID is {string}, the first 12 hex characters of the SHA-256 of the pak")]
fn id_is(w: &mut ImportWorld, id: String) {
    assert_eq!(w.entries()[0].id, id);
}

#[then("library.json and library/<id>/A.pak contain the imported mod")]
fn json_and_folder(w: &mut ImportWorld) {
    let entry = w.entries().remove(0);
    let data = w.data();
    let json = fs::read_to_string(data.library_file()).unwrap();
    assert!(json.contains(&entry.id), "library.json: {json}");
    assert_eq!(w.index(), vec![entry.clone()]);
    assert!(data.library_dir().join(&entry.id).join("A.pak").is_file());
}

// ---------- Scenario: Pick up only the siblings ----------

#[given(expr = "{string}, {string}, {string} and {string} in the same folder")]
fn siblings(w: &mut ImportWorld, a: String, b: String, c: String, d: String) {
    let src = w.src();
    for (name, content) in [(a, "pak"), (b, "utoc"), (c, "ucas"), (d, "other")] {
        fs::write(src.join(name), content).unwrap();
    }
}

#[when(expr = "I import {string}")]
fn import_named(w: &mut ImportWorld, name: String) {
    w.source = w.src().join(name);
    w.import();
}

#[then(expr = "the mod is named {string} and has the three files of A_P")]
fn named_with_three(w: &mut ImportWorld, name: String) {
    let entries = w.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, name);
    assert_eq!(entries[0].files, vec!["A_P.pak", "A_P.ucas", "A_P.utoc"]);
}

#[then("B is not imported")]
fn b_not_imported(w: &mut ImportWorld) {
    let index = w.index();
    assert_eq!(index.len(), 1);
    assert_ne!(index[0].name, "B");
}

// ---------- Scenarios with ZIPs ----------

#[given(expr = "a ZIP containing {string} and {string}")]
fn zip_with_two(w: &mut ImportWorld, a: String, b: String) {
    let zip = w.src().join("mods.zip");
    zips::build_zip(&zip, &[(a.as_str(), b"abc"), (b.as_str(), b"bbb")]);
    w.source = zip;
}

#[given(expr = "a ZIP containing {string} and {string} with different contents")]
fn zip_with_collision(w: &mut ImportWorld, a: String, b: String) {
    let zip = w.src().join("collide.zip");
    zips::build_zip(&zip, &[(a.as_str(), b"one"), (b.as_str(), b"two")]);
    w.source = zip;
}

#[given(expr = "a ZIP containing only {string}")]
fn zip_only(w: &mut ImportWorld, name: String) {
    let zip = w.src().join("docs.zip");
    zips::build_zip(&zip, &[(name.as_str(), b"hi")]);
    w.source = zip;
}

#[then("I receive two entries with flattened file names")]
fn two_entries(w: &mut ImportWorld) {
    let entries = w.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].files, vec!["A.pak"]);
    assert_eq!(entries[1].files, vec!["B.pak"]);
}

#[then(expr = "I receive an error containing {string}")]
fn error_containing(w: &mut ImportWorld, text: String) {
    let error = w
        .result
        .clone()
        .expect("import not run")
        .expect_err("expected an error");
    assert!(error.to_string().contains(&text), "got: {error}");
}

// ---------- Scenario: Reimport ----------

#[given("a pak that was already imported")]
fn already_imported(w: &mut ImportWorld) {
    file_with_content(w, "A.pak".into(), "abc".into());
    let data = w.data();
    w.first = Some(mod_library(&data).import(&w.source).unwrap());
}

#[when("I import the same content again")]
fn import_again(w: &mut ImportWorld) {
    w.import();
}

#[then("I receive the existing entry")]
fn existing_entry(w: &mut ImportWorld) {
    assert_eq!(Some(w.entries()), w.first.clone());
}

#[then("the library still has one entry for that ID")]
fn one_entry(w: &mut ImportWorld) {
    let id = w.first.clone().unwrap()[0].id.clone();
    let count = w.index().iter().filter(|e| e.id == id).count();
    assert_eq!(count, 1);
    assert_eq!(w.index().len(), 1);
}

// ---------- Scenario: Remove ----------

#[given("an imported mod")]
fn imported_mod(w: &mut ImportWorld) {
    already_imported(w);
}

#[when("I remove its ID")]
fn remove_id(w: &mut ImportWorld) {
    w.removed_id = w.first.clone().unwrap()[0].id.clone();
    let data = w.data();
    mod_library(&data).remove(&w.removed_id).unwrap();
}

#[then("its folder and index entry no longer exist")]
fn gone(w: &mut ImportWorld) {
    let data = w.data();
    assert!(!data.library_dir().join(&w.removed_id).exists());
    assert!(w.index().is_empty());
}

// ---------- Scenario: Traversal ----------

#[then(expr = "only {string} enters the library")]
fn only_enters(w: &mut ImportWorld, name: String) {
    let entries = w.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].files, vec![name]);
    assert_eq!(w.index().len(), 1);
}

#[then("no file is written outside the controlled directory")]
fn nothing_outside(w: &mut ImportWorld) {
    let root = w.root();
    let data = w.data();
    for dir in [
        root.join("downloads"),
        root.clone(),
        data.root().to_path_buf(),
    ] {
        assert!(!dir.join("evil.pak").exists(), "evil.pak in {dir:?}");
    }
    assert!(!data.library_dir().join("evil.pak").exists());
    assert!(!w.tmp.as_ref().unwrap().path().join("evil.pak").exists());
}

// ---------- Scenario: Collision ----------

#[given("a mod already in the library")]
fn mod_already(w: &mut ImportWorld) {
    let zip_source = w.source.clone();
    let existing = w.src().join("Keep.pak");
    fs::write(&existing, "keep").unwrap();
    let data = w.data();
    w.first = Some(mod_library(&data).import(&existing).unwrap());
    w.library_file_before = fs::read(data.library_file()).unwrap();
    w.source = zip_source;
}

#[then("I receive a collision error")]
fn collision_error(w: &mut ImportWorld) {
    assert!(
        matches!(w.result, Some(Err(LibraryError::NameCollision { .. }))),
        "got {:?}",
        w.result
    );
}

#[then("no existing library file is overwritten")]
fn nothing_overwritten(w: &mut ImportWorld) {
    let existing = w.first.clone().unwrap().remove(0);
    let data = w.data();
    assert_eq!(
        fs::read(data.library_dir().join(&existing.id).join("Keep.pak")).unwrap(),
        b"keep"
    );
    assert_eq!(
        fs::read(data.library_file()).unwrap(),
        w.library_file_before
    );
    let folders = fs::read_dir(data.library_dir()).unwrap().count();
    assert_eq!(folders, 1, "no new folder or staging leftovers");
}

// ---------- Scenario: Unsupported format ----------

#[when("I try to import it")]
fn try_import(w: &mut ImportWorld) {
    w.import();
}

/// Runs only `file-import.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ImportWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("file-import.feature"))
        .await;
    !writer.execution_has_failed()
}
