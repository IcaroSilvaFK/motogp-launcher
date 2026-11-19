//! Steps for `tests/features/deploy.feature` (issue #6), using in-memory fakes
//! of the deploy ports. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{DeployError, DeployMod};
use motogp_domain::Deployed;
use std::path::PathBuf;

#[path = "../support/deploy_fakes.rs"]
mod fakes;
use fakes::{
    disk_has, disk_names, mod_of, mods_dir, new_state, put_disk, put_sources, service, Shared,
    ID_A, ID_B,
};

#[derive(Debug, World)]
#[world(init = Self::new)]
struct DeployWorld {
    state: Shared,
    deployed: Vec<String>,
    deploy_result: Option<Result<Vec<String>, DeployError>>,
    clean_result: Option<Result<usize, DeployError>>,
}

impl DeployWorld {
    fn new() -> Self {
        Self {
            state: new_state(),
            deployed: vec![],
            deploy_result: None,
            clean_result: None,
        }
    }

    fn deploy(&mut self, mods: &[DeployMod]) {
        put_sources(&self.state, mods);
        self.deploy_result = Some(service(&self.state).deploy(&mods_dir(), mods));
    }

    fn deploy_ok(&mut self, mods: &[DeployMod]) {
        self.deploy(mods);
        self.deployed = self.deploy_result.clone().unwrap().expect("deploy failed");
    }
}

fn placeholders(text: &str) -> String {
    text.replace("<idA>", ID_A).replace("<idB>", ID_B)
}

// ---------- Deploy in the profile order ----------

#[given("the mods B and A in that order")]
fn mods_b_and_a(_: &mut DeployWorld) {}

#[when("I deploy their files")]
fn deploy_their_files(w: &mut DeployWorld) {
    w.deploy_ok(&[mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])]);
}

#[then(expr = "the destinations are {string} and {string}")]
fn destinations(w: &mut DeployWorld, a: String, b: String) {
    assert_eq!(w.deployed, vec![placeholders(&a), placeholders(&b)]);
    assert!(w.deployed.iter().all(|n| disk_has(&w.state, n)));
}

#[then("each deployed file has a recoverable persisted record")]
fn persisted_record(w: &mut DeployWorld) {
    assert_eq!(w.state.borrow().manifest.files, w.deployed);
    assert_eq!(w.state.borrow().manifest.mods_dir, mods_dir());
}

// ---------- Clean only what was deployed / missing file / partial failure ----------

#[given(expr = "registered deployed files and the manual file {string} in ~mods")]
fn registered_and_manual(w: &mut DeployWorld, manual: String) {
    w.deploy_ok(&[mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])]);
    put_disk(&w.state, &manual, b"manual-bytes");
}

#[given("a registered file was already removed")]
fn registered_removed(w: &mut DeployWorld) {
    w.deploy_ok(&[mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])]);
    w.state
        .borrow_mut()
        .disk
        .remove(&(mods_dir(), w.deployed[0].clone()));
}

#[given("two registered files and a failure when removing the second")]
fn failing_second(w: &mut DeployWorld) {
    w.deploy_ok(&[mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])]);
    w.state.borrow_mut().fail_remove = vec![w.deployed[1].clone()];
}

#[when("I clean the session")]
fn clean_session(w: &mut DeployWorld) {
    w.clean_result = Some(service(&w.state).clean());
}

#[then("only the registered files are removed")]
fn only_registered_removed(w: &mut DeployWorld) {
    assert_eq!(disk_names(&w.state), vec!["meu_mod.pak"]);
}

#[then(expr = "{word} keeps the same bytes")]
fn manual_bytes(w: &mut DeployWorld, name: String) {
    assert_eq!(
        w.state.borrow().disk[&(mods_dir(), name)],
        b"manual-bytes".to_vec()
    );
}

#[then("the manifest is empty after a complete cleanup")]
fn manifest_empty(w: &mut DeployWorld) {
    assert!(w.clean_result.clone().unwrap().is_ok());
    assert_eq!(w.state.borrow().manifest, Deployed::default());
}

#[then("the cleanup finishes without an error for that file")]
fn clean_ok(w: &mut DeployWorld) {
    assert!(w.clean_result.clone().unwrap().is_ok());
}

#[then("the count considers only files actually removed")]
fn count_only_removed(w: &mut DeployWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 1);
}

#[then("I get an error")]
fn get_error(w: &mut DeployWorld) {
    assert!(
        matches!(w.clean_result, Some(Err(_))),
        "{:?}",
        w.clean_result
    );
}

#[then("the second file remains tracked for recovery")]
fn second_tracked(w: &mut DeployWorld) {
    let manifest = w.state.borrow().manifest.clone();
    assert_eq!(manifest.files, vec![w.deployed[1].clone()]);
}

// ---------- Clean the previous session before redeploying ----------

#[given("a previous session with A deployed")]
fn previous_a(w: &mut DeployWorld) {
    w.deploy_ok(&[mod_of(ID_A, &["A.pak"])]);
    w.state.borrow_mut().log.clear();
}

#[when("I deploy only B")]
fn deploy_only_b(w: &mut DeployWorld) {
    w.deploy_ok(&[mod_of(ID_B, &["B.pak"])]);
}

#[then("the files of A are removed before those of B")]
fn a_before_b(w: &mut DeployWorld) {
    let log = w.state.borrow().log.clone();
    assert_eq!(
        log,
        vec![
            "remove 000_aaaaaaaaaaaa_A.pak".to_string(),
            "materialize 000_bbbbbbbbbbbb_B.pak".to_string()
        ]
    );
}

#[then("only B is registered")]
fn only_b_registered(w: &mut DeployWorld) {
    assert_eq!(w.state.borrow().manifest.files, w.deployed);
    assert_eq!(disk_names(&w.state), vec!["000_bbbbbbbbbbbb_B.pak"]);
}

// ---------- Refuse a collision ----------

#[given("a manual file with the exact name of the planned destination")]
fn manual_collision(w: &mut DeployWorld) {
    put_disk(&w.state, "000_aaaaaaaaaaaa_A.pak", b"manual-bytes");
}

#[when("I try to deploy the mod")]
fn try_deploy(w: &mut DeployWorld) {
    w.deploy(&[mod_of(ID_A, &["A.pak"])]);
}

#[then(expr = "I get an error containing {string}")]
fn error_containing(w: &mut DeployWorld, text: String) {
    let error = w
        .deploy_result
        .clone()
        .expect("deploy not run")
        .expect_err("expected an error");
    assert!(error.to_string().contains(&text), "got: {error}");
}

#[then("the manual file stays intact and outside the cleanup")]
fn manual_intact(w: &mut DeployWorld) {
    assert!(w.state.borrow().manifest.is_empty());
    service(&w.state).clean().unwrap();
    assert_eq!(
        w.state.borrow().disk[&(mods_dir(), "000_aaaaaaaaaaaa_A.pak".to_string())],
        b"manual-bytes".to_vec()
    );
}

// ---------- Empty profile ----------

#[given("there is no previous session nor a ~mods folder")]
fn no_session(_: &mut DeployWorld) {}

#[when("I deploy a profile without mods")]
fn deploy_empty(w: &mut DeployWorld) {
    w.deploy_ok(&[]);
}

#[then("~mods remains absent")]
fn mods_absent(w: &mut DeployWorld) {
    assert!(w.state.borrow().dirs.is_empty());
    assert!(w.state.borrow().disk.is_empty());
}

// ---------- Interruption ----------

#[given("an interruption at a point between the record and the materialization of a file")]
fn interruption(w: &mut DeployWorld) {
    put_disk(&w.state, "meu_mod.pak", b"manual-bytes");
    let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
    put_sources(&w.state, &mods);
    w.state.borrow_mut().crash_after_materialize = Some(1);
    assert!(service(&w.state).deploy(&mods_dir(), &mods).is_err());
}

#[when("I resume recovery from the manifest and the journal")]
fn resume(w: &mut DeployWorld) {
    // The new process starts alive and recovers from the persisted state.
    w.state.borrow_mut().dead = false;
    w.clean_result = Some(service(&w.state).clean());
}

#[then("every file proven deployed can be cleaned")]
fn proven_cleaned(w: &mut DeployWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 2);
    assert!(w.state.borrow().manifest.is_empty());
}

#[then("no foreign file is deleted")]
fn foreign_kept(w: &mut DeployWorld) {
    assert_eq!(disk_names(&w.state), vec!["meu_mod.pak"]);
}

// ---------- Tampered manifest ----------

#[given(expr = "a tampered manifest with a path containing {string}")]
fn tampered(w: &mut DeployWorld, fragment: String) {
    w.state.borrow_mut().disk.insert(
        (PathBuf::from("game"), "evil.pak".to_string()),
        b"external".to_vec(),
    );
    w.state.borrow_mut().manifest = Deployed {
        mods_dir: mods_dir(),
        files: vec![format!("{fragment}/evil.pak")],
        pending: vec![],
    };
}

#[when("I try to clean the session")]
fn try_clean(w: &mut DeployWorld) {
    w.clean_result = Some(service(&w.state).clean());
}

#[then("the operation returns an error")]
fn operation_error(w: &mut DeployWorld) {
    assert!(matches!(
        w.clean_result,
        Some(Err(DeployError::InvalidPath { .. }))
    ));
}

#[then("no external file is removed")]
fn external_kept(w: &mut DeployWorld) {
    assert!(w
        .state
        .borrow()
        .disk
        .contains_key(&(PathBuf::from("game"), "evil.pak".to_string())));
}

/// Runs only `deploy.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = DeployWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("deploy.feature"))
        .await;
    !writer.execution_has_failed()
}
