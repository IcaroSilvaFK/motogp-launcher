//! Steps for `tests/features/file-deploy.feature` (issue #6): real fs adapters
//! in temporary directories. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{DeployError, DeployMod, DeployService, DeployedRepository};
use motogp_domain::Deployed;
use motogp_infrastructure::{FsDeployFiles, JsonDeployedRepository};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ID_A: &str = "aaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbb";

fn refusing_link(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "cross-volume link",
    ))
}

#[derive(Debug, Default, World)]
struct FileWorld {
    tmp: Option<TempDir>,
    deployed: Vec<String>,
    deploy_errors: Vec<String>,
    clean_result: Option<Result<usize, DeployError>>,
    log_order: Vec<String>,
}

impl FileWorld {
    fn root(&mut self) -> PathBuf {
        self.tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
            .path()
            .join("Usuário Padrão")
    }

    fn mods_dir(&mut self) -> PathBuf {
        self.root().join("game").join("~mods")
    }

    fn manifest_path(&mut self) -> PathBuf {
        self.root().join("data").join("deployed.json")
    }

    fn repo(&mut self) -> JsonDeployedRepository {
        JsonDeployedRepository::new(self.manifest_path())
    }

    fn mod_of(&mut self, id: &str, file: &str) -> DeployMod {
        let dir = self.root().join("lib").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), file.as_bytes()).unwrap();
        DeployMod {
            id: id.into(),
            source_dir: dir,
            files: vec![file.into()],
        }
    }

    fn deploy_with(
        &mut self,
        files: FsDeployFiles,
        mods: &[DeployMod],
    ) -> Result<Vec<String>, DeployError> {
        let mods_dir = self.mods_dir();
        DeployService::new(self.repo(), files).deploy(&mods_dir, mods)
    }

    fn deploy(&mut self, mods: &[DeployMod]) {
        self.deployed = self
            .deploy_with(FsDeployFiles::new(), mods)
            .expect("deploy failed");
    }

    fn clean(&mut self) -> Result<usize, DeployError> {
        DeployService::new(self.repo(), FsDeployFiles::new()).clean()
    }

    fn two_mods(&mut self) -> [DeployMod; 2] {
        [self.mod_of(ID_B, "B.pak"), self.mod_of(ID_A, "A.pak")]
    }

    fn manifest(&mut self) -> Deployed {
        self.repo().load().unwrap()
    }

    fn mods_names(&mut self) -> Vec<String> {
        let dir = self.mods_dir();
        let Ok(read) = fs::read_dir(dir) else {
            return vec![];
        };
        let mut names: Vec<String> = read
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

fn placeholders(text: &str) -> String {
    text.replace("<idA>", ID_A).replace("<idB>", ID_B)
}

// ---------- Deploy in the profile order ----------

#[given("the mods B and A in that order")]
fn mods_b_and_a(_: &mut FileWorld) {}

#[when("I deploy their files")]
fn deploy_their_files(w: &mut FileWorld) {
    let mods = w.two_mods();
    w.deploy(&mods);
}

#[then(expr = "the destinations are {string} and {string}")]
fn destinations(w: &mut FileWorld, a: String, b: String) {
    let expected = vec![placeholders(&a), placeholders(&b)];
    assert_eq!(w.deployed, expected);
    let mut sorted = expected.clone();
    sorted.sort();
    assert_eq!(w.mods_names(), sorted);
    let dir = w.mods_dir();
    assert_eq!(fs::read(dir.join(&expected[0])).unwrap(), b"B.pak");
}

#[then("each deployed file has a recoverable persisted record")]
fn persisted(w: &mut FileWorld) {
    let manifest = w.manifest();
    assert_eq!(manifest.files, w.deployed);
    assert_eq!(manifest.mods_dir, w.mods_dir());
}

// ---------- Clean ----------

#[given(expr = "registered deployed files and the manual file {string} in ~mods")]
fn registered_and_manual(w: &mut FileWorld, manual: String) {
    let mods = w.two_mods();
    w.deploy(&mods);
    let dir = w.mods_dir();
    fs::write(dir.join(manual), b"manual-bytes").unwrap();
}

#[given("a registered file was already removed")]
fn registered_removed(w: &mut FileWorld) {
    let mods = w.two_mods();
    w.deploy(&mods);
    let dir = w.mods_dir();
    fs::remove_file(dir.join(&w.deployed[0])).unwrap();
}

#[given("two registered files and a failure when removing the second")]
fn failing_second(w: &mut FileWorld) {
    let mods = w.two_mods();
    w.deploy(&mods);
    // Replace the second deployed file with a non-empty directory: removing a
    // file entry that is a directory fails on every platform.
    let dir = w.mods_dir();
    let second = dir.join(&w.deployed[1]);
    fs::remove_file(&second).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(second.join("inner"), b"x").unwrap();
}

#[when("I clean the session")]
fn clean_session(w: &mut FileWorld) {
    w.clean_result = Some(w.clean());
}

#[then("only the registered files are removed")]
fn only_registered(w: &mut FileWorld) {
    assert_eq!(w.mods_names(), vec!["meu_mod.pak"]);
}

#[then(expr = "{word} keeps the same bytes")]
fn manual_bytes(w: &mut FileWorld, name: String) {
    let dir = w.mods_dir();
    assert_eq!(fs::read(dir.join(name)).unwrap(), b"manual-bytes");
}

#[then("the manifest is empty after a complete cleanup")]
fn manifest_empty(w: &mut FileWorld) {
    assert!(w.clean_result.clone().unwrap().is_ok());
    assert_eq!(w.manifest(), Deployed::default());
}

#[then("the cleanup finishes without an error for that file")]
fn clean_ok(w: &mut FileWorld) {
    assert!(w.clean_result.clone().unwrap().is_ok());
}

#[then("the count considers only files actually removed")]
fn count(w: &mut FileWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 1);
}

#[then("I get an error")]
fn get_error(w: &mut FileWorld) {
    assert!(
        matches!(w.clean_result, Some(Err(_))),
        "{:?}",
        w.clean_result
    );
}

#[then("the second file remains tracked for recovery")]
fn second_tracked(w: &mut FileWorld) {
    let manifest = w.manifest();
    assert_eq!(manifest.files, vec![w.deployed[1].clone()]);
}

// ---------- Clean previous session ----------

#[given("a previous session with A deployed")]
fn previous_a(w: &mut FileWorld) {
    let a = w.mod_of(ID_A, "A.pak");
    w.deploy(&[a]);
    w.log_order = w.deployed.clone();
}

#[when("I deploy only B")]
fn deploy_only_b(w: &mut FileWorld) {
    let b = w.mod_of(ID_B, "B.pak");
    w.deploy(&[b]);
}

#[then("the files of A are removed before those of B")]
fn a_removed(w: &mut FileWorld) {
    // The index restarts at 000 and only B exists: A's file is gone.
    let names = w.mods_names();
    assert!(names.iter().all(|n| !n.contains(ID_A)), "{names:?}");
    assert_eq!(w.log_order.len(), 1);
}

#[then("only B is registered")]
fn only_b(w: &mut FileWorld) {
    assert_eq!(w.manifest().files, w.deployed);
    assert_eq!(w.deployed, vec!["000_bbbbbbbbbbbb_B.pak"]);
    assert_eq!(w.mods_names(), w.deployed);
}

// ---------- Collision ----------

#[given("a manual file with the exact name of the planned destination")]
fn manual_collision(w: &mut FileWorld) {
    let dir = w.mods_dir();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("000_aaaaaaaaaaaa_A.pak"), b"manual-bytes").unwrap();
}

#[when("I try to deploy the mod by hard link or copy fallback")]
fn try_deploy(w: &mut FileWorld) {
    let a = w.mod_of(ID_A, "A.pak");
    // Hard link path: the adapter itself must refuse; copy path: link always fails.
    for files in [
        FsDeployFiles::new(),
        FsDeployFiles::with_link(refusing_link),
    ] {
        let dir = w.mods_dir();
        let direct = motogp_application::DeployFiles::materialize(
            &files,
            &a.source_dir.join("A.pak"),
            &dir,
            "000_aaaaaaaaaaaa_A.pak",
        )
        .unwrap_err();
        w.deploy_errors.push(direct.to_string());
        let through_service = w.deploy_with(files, std::slice::from_ref(&a)).unwrap_err();
        w.deploy_errors.push(through_service.to_string());
    }
}

#[then(expr = "I get an error containing {string}")]
fn error_containing(w: &mut FileWorld, text: String) {
    assert_eq!(w.deploy_errors.len(), 4);
    for error in &w.deploy_errors {
        assert!(error.contains(&text), "got: {error}");
    }
}

#[then("the manual file stays intact and outside the cleanup")]
fn manual_intact(w: &mut FileWorld) {
    let dir = w.mods_dir();
    assert_eq!(
        fs::read(dir.join("000_aaaaaaaaaaaa_A.pak")).unwrap(),
        b"manual-bytes"
    );
    assert!(w.manifest().is_empty());
    w.clean().unwrap();
    assert_eq!(w.mods_names(), vec!["000_aaaaaaaaaaaa_A.pak"]);
}

// ---------- Empty profile ----------

#[given("there is no previous session nor a ~mods folder")]
fn no_session(_: &mut FileWorld) {}

#[when("I deploy a profile without mods")]
fn deploy_empty(w: &mut FileWorld) {
    w.deploy(&[]);
}

#[then("~mods remains absent")]
fn mods_absent(w: &mut FileWorld) {
    assert!(!w.mods_dir().exists());
}

// ---------- Interruption ----------

#[given("an interruption at a point between the record and the materialization of a file")]
fn interruption(w: &mut FileWorld) {
    // State left on disk by a process that died: one file fully deployed and
    // recorded, a second recorded as pending that did materialize, and a third
    // recorded as pending that never did.
    let dir = w.mods_dir();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("meu_mod.pak"), b"manual-bytes").unwrap();
    fs::write(dir.join("000_bbbbbbbbbbbb_B.pak"), b"B").unwrap();
    fs::write(dir.join("001_aaaaaaaaaaaa_A.pak"), b"A").unwrap();
    let manifest = Deployed {
        mods_dir: dir,
        files: vec!["000_bbbbbbbbbbbb_B.pak".into()],
        pending: vec![
            "001_aaaaaaaaaaaa_A.pak".into(),
            "002_cccccccccccc_C.pak".into(),
        ],
    };
    w.repo().save(&manifest).unwrap();
}

#[when("I resume recovery from the manifest and the journal")]
fn resume(w: &mut FileWorld) {
    w.clean_result = Some(w.clean());
}

#[then("every file proven deployed can be cleaned")]
fn proven_cleaned(w: &mut FileWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 2);
    assert!(w.manifest().is_empty());
}

#[then("no foreign file is deleted")]
fn foreign_kept(w: &mut FileWorld) {
    assert_eq!(w.mods_names(), vec!["meu_mod.pak"]);
}

// ---------- Tampered manifest ----------

#[given(expr = "a tampered manifest with a path containing {string}")]
fn tampered(w: &mut FileWorld, fragment: String) {
    let dir = w.mods_dir();
    fs::create_dir_all(&dir).unwrap();
    let external = dir.parent().unwrap().join("evil.pak");
    fs::write(&external, b"external").unwrap();
    let manifest = Deployed {
        mods_dir: dir,
        files: vec![format!("{fragment}/evil.pak")],
        pending: vec![],
    };
    w.repo().save(&manifest).unwrap();
}

#[when("I try to clean the session")]
fn try_clean(w: &mut FileWorld) {
    w.clean_result = Some(w.clean());
}

#[then("the operation returns an error")]
fn operation_error(w: &mut FileWorld) {
    assert!(matches!(
        w.clean_result,
        Some(Err(DeployError::InvalidPath { .. }))
    ));
}

#[then("no external file is removed")]
fn external_kept(w: &mut FileWorld) {
    let external = w.mods_dir().parent().unwrap().join("evil.pak");
    assert_eq!(fs::read(external).unwrap(), b"external");
}

/// Runs only `file-deploy.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = FileWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("file-deploy.feature"))
        .await;
    !writer.execution_has_failed()
}
