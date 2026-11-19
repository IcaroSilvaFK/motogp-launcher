//! Steps for `tests/features/stale-manifest.feature` (issue #23): real fs
//! adapters in temporary directories, two game installs.
//! Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    DeployError, DeployMod, DeployService, DeploySession, DeployedRepository,
};
use motogp_domain::Deployed;
use motogp_infrastructure::{FsDeployFiles, JsonDeployedRepository};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ID_A: &str = "aaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbb";

type Snapshot = BTreeMap<PathBuf, Vec<u8>>;

#[derive(Debug, Default, World)]
struct StaleWorld {
    tmp: Option<TempDir>,
    /// Folder recorded in the manifest (set by the Given steps).
    recorded: Option<PathBuf>,
    /// Names of the files deployed by the launcher into `recorded`.
    deployed: Vec<String>,
    session: Option<DeploySession>,
    snapshot: Option<(Snapshot, Deployed)>,
    clean_result: Option<Result<usize, DeployError>>,
    deploy_result: Option<Result<Vec<String>, DeployError>>,
}

impl StaleWorld {
    fn root(&mut self) -> PathBuf {
        self.tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
            .path()
            // Accented folder on purpose: real Windows users have them.
            .join("Usuário Padrão")
    }

    fn old(&mut self) -> PathBuf {
        self.root().join("install-old").join("~mods")
    }

    fn current(&mut self) -> PathBuf {
        self.root().join("install-new").join("~mods")
    }

    fn repo(&mut self) -> JsonDeployedRepository {
        JsonDeployedRepository::new(self.root().join("data").join("deployed.json"))
    }

    fn service(&mut self) -> DeployService<JsonDeployedRepository, FsDeployFiles> {
        DeployService::new(self.repo(), FsDeployFiles::new())
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

    /// Deploys two mods into `dir` as if it was the configured install, then
    /// makes the current install's `~mods` exist as well.
    fn deploy_into(&mut self, dir: &Path) {
        let mods = [self.mod_of(ID_B, "B.pak"), self.mod_of(ID_A, "A.pak")];
        self.deployed = self.service().deploy(dir, &mods).expect("deploy failed");
        self.recorded = Some(dir.to_path_buf());
        let current = self.current();
        fs::create_dir_all(current).unwrap();
    }

    fn recorded(&self) -> PathBuf {
        self.recorded.clone().expect("no recorded folder")
    }

    /// Every file under the install roots with its bytes.
    fn files(&mut self) -> Snapshot {
        let mut out = Snapshot::new();
        for dir in [
            self.root().join("install-old"),
            self.root().join("install-new"),
        ] {
            collect(&dir, &mut out);
        }
        let documents = self.root().join("install-old").join("Documents");
        collect(&documents, &mut out);
        out
    }

    fn snapshot(&mut self) {
        if self.snapshot.is_none() {
            let files = self.files();
            let manifest = self.repo().load().unwrap();
            self.snapshot = Some((files, manifest));
        }
    }
}

fn collect(dir: &Path, out: &mut Snapshot) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.insert(path.clone(), fs::read(&path).unwrap());
        }
    }
}

#[given("the authorized ~mods of the current game install")]
fn authorized(_: &mut StaleWorld) {}

#[given("a deploy manifest that points to a previous game install")]
fn previous_install(w: &mut StaleWorld) {
    let old = w.old();
    w.deploy_into(&old);
}

#[given(expr = "a deploy manifest that points to a folder not named {string}")]
fn not_mods(w: &mut StaleWorld, _name: String) {
    let dir = w.root().join("install-old").join("Documents");
    w.deploy_into(&dir);
}

#[given("a deploy manifest that points to a folder that was deleted")]
fn deleted_folder(w: &mut StaleWorld) {
    let old = w.old();
    w.deploy_into(&old);
    fs::remove_dir_all(old).unwrap();
}

#[given("a deploy manifest that points to the authorized ~mods")]
fn current_install(w: &mut StaleWorld) {
    let current = w.current();
    w.deploy_into(&current);
}

#[given(expr = "the user file {string} in the old folder")]
fn user_file(w: &mut StaleWorld, name: String) {
    fs::write(w.old().join(name), b"manual-bytes").unwrap();
}

#[given("a tracked file in the old folder was modified after the deploy")]
fn modified(w: &mut StaleWorld) {
    // Replace the file (a hard link edited in place would still be ours).
    let path = w.old().join(&w.deployed[0]);
    fs::remove_file(&path).unwrap();
    fs::write(path, b"edited").unwrap();
}

#[when("I inspect the deploy session")]
fn inspect(w: &mut StaleWorld) {
    w.snapshot();
    let current = w.current();
    w.session = Some(w.service().session(&current).unwrap());
}

#[when("the user chooses to forget the old session")]
fn forget(w: &mut StaleWorld) {
    w.snapshot();
    let current = w.current();
    w.service()
        .forget_stale_session(&current)
        .expect("forget failed");
}

#[when("the user confirms cleaning the old folder")]
fn confirm_clean(w: &mut StaleWorld) {
    w.snapshot();
    let (current, confirmed) = (w.current(), w.recorded());
    w.clean_result = Some(w.service().clean_stale_session(&current, &confirmed));
}

#[when("the user confirms cleaning a different folder")]
fn confirm_other(w: &mut StaleWorld) {
    w.snapshot();
    let current = w.current();
    w.clean_result = Some(w.service().clean_stale_session(&current, &current));
}

#[when("I deploy a profile into the authorized ~mods")]
fn deploy(w: &mut StaleWorld) {
    let current = w.current();
    let mods = [w.mod_of(ID_A, "A.pak")];
    w.deploy_result = Some(w.service().deploy(&current, &mods));
}

#[then("it is reported as stale with the old folder and its tracked file count")]
fn reported_stale(w: &mut StaleWorld) {
    let old = w.old();
    assert_eq!(
        w.session,
        Some(DeploySession::Stale {
            mods_dir: old,
            tracked: 2
        })
    );
}

#[then("nothing is changed on disk or in the manifest")]
fn nothing_changed(w: &mut StaleWorld) {
    let (files, manifest) = w.snapshot.clone().unwrap();
    assert_eq!(w.files(), files);
    assert_eq!(w.repo().load().unwrap(), manifest);
}

#[then("it is reported as vanished")]
fn reported_vanished(w: &mut StaleWorld) {
    let old = w.old();
    assert_eq!(w.session, Some(DeploySession::Vanished { mods_dir: old }));
}

#[then("it is reported as current")]
fn reported_current(w: &mut StaleWorld) {
    assert_eq!(w.session, Some(DeploySession::Current { tracked: 2 }));
}

#[then("the manifest is cleared")]
fn manifest_cleared(w: &mut StaleWorld) {
    assert_eq!(w.repo().load().unwrap(), Deployed::default());
}

#[then("no file is deleted")]
fn no_file_deleted(w: &mut StaleWorld) {
    let (files, _) = w.snapshot.clone().unwrap();
    assert_eq!(w.files(), files);
}

#[then("only files proven to be ours are deleted from that folder")]
fn ours_deleted(w: &mut StaleWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 2);
    let recorded = w.recorded();
    assert!(w.deployed.iter().all(|name| !recorded.join(name).exists()));
}

#[then(expr = "{string} keeps the same bytes")]
fn same_bytes(w: &mut StaleWorld, name: String) {
    assert_eq!(fs::read(w.old().join(name)).unwrap(), b"manual-bytes");
}

#[then("the modified file keeps its new bytes")]
fn modified_kept(w: &mut StaleWorld) {
    assert_eq!(fs::read(w.old().join(&w.deployed[0])).unwrap(), b"edited");
}

#[then("the cleanup is refused as unauthorized")]
fn refused(w: &mut StaleWorld) {
    assert!(matches!(
        w.clean_result,
        Some(Err(DeployError::UnauthorizedDirectory { .. }))
    ));
}

#[then("the manifest is kept")]
fn manifest_kept(w: &mut StaleWorld) {
    let (_, manifest) = w.snapshot.clone().unwrap();
    assert_eq!(w.repo().load().unwrap(), manifest);
}

#[then("the manifest is cleared without an error")]
fn cleared_without_error(w: &mut StaleWorld) {
    assert_eq!(w.clean_result, Some(Ok(0)));
    manifest_cleared(w);
}

#[then("forgetting it is refused as not stale and the manifest is kept")]
fn forget_refused(w: &mut StaleWorld) {
    let current = w.current();
    let result = w.service().forget_stale_session(&current);
    assert!(matches!(result, Err(DeployError::SessionNotStale { .. })));
    manifest_kept(w);
}

#[then("deploying cleans it as before")]
fn deploy_cleans(w: &mut StaleWorld) {
    deploy(w);
    assert!(w.deploy_result.clone().unwrap().is_ok());
    let current = w.current();
    assert!(!current.join(&w.deployed[0]).exists());
}

#[then("the deploy succeeds")]
fn deploy_succeeds(w: &mut StaleWorld) {
    assert_eq!(
        w.deploy_result.clone().unwrap().unwrap(),
        vec!["000_aaaaaaaaaaaa_A.pak"]
    );
}

/// Runs only `stale-manifest.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = StaleWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("stale-manifest.feature"))
        .await;
    !writer.execution_has_failed()
}
