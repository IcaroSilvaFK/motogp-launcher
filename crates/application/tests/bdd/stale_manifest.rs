//! Steps for `tests/features/stale-manifest.feature` (issue #23), using
//! in-memory fakes of the deploy ports. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{DeployError, DeploySession};
use motogp_domain::Deployed;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::deploy_fakes as fakes;
use fakes::{
    disk_has_in, manifest_names, mod_of, mods_dir, new_state, old_mods_dir, put_disk_in,
    put_sources, service, stale_manifest, Shared, ID_A,
};

const TRACKED: [&str; 2] = ["000_old_A.pak", "001_old_B.pak"];

#[derive(Debug, World)]
#[world(init = Self::new)]
struct StaleWorld {
    state: Shared,
    /// Folder recorded in the manifest.
    recorded: PathBuf,
    session: Option<DeploySession>,
    /// Disk and manifest right before the first action.
    disk_before: Option<BTreeMap<(PathBuf, String), Vec<u8>>>,
    manifest_before: Option<Deployed>,
    clean_result: Option<Result<usize, DeployError>>,
    deploy_result: Option<Result<Vec<String>, DeployError>>,
}

impl StaleWorld {
    fn new() -> Self {
        Self {
            state: new_state(),
            recorded: old_mods_dir(),
            session: None,
            disk_before: None,
            manifest_before: None,
            clean_result: None,
            deploy_result: None,
        }
    }

    fn snapshot(&mut self) {
        if self.disk_before.is_none() {
            self.disk_before = Some(self.state.borrow().disk.clone());
            self.manifest_before = Some(self.state.borrow().manifest.clone());
        }
    }
}

#[given("the authorized ~mods of the current game install")]
fn authorized(_: &mut StaleWorld) {}

#[given("a deploy manifest that points to a previous game install")]
fn previous_install(w: &mut StaleWorld) {
    stale_manifest(&w.state, &old_mods_dir(), &TRACKED);
}

#[given(expr = "a deploy manifest that points to a folder not named {string}")]
fn not_mods(w: &mut StaleWorld, _name: String) {
    w.recorded = PathBuf::from("oldgame/Documents");
    stale_manifest(&w.state, &w.recorded, &TRACKED);
}

#[given("a deploy manifest that points to a folder that was deleted")]
fn deleted_folder(w: &mut StaleWorld) {
    stale_manifest(&w.state, &old_mods_dir(), &TRACKED);
    w.state.borrow_mut().disk.clear();
    w.state.borrow_mut().missing_dirs.push(old_mods_dir());
}

#[given("a deploy manifest that points to the authorized ~mods")]
fn current_install(w: &mut StaleWorld) {
    stale_manifest(&w.state, &mods_dir(), &TRACKED);
}

#[given(expr = "the user file {string} in the old folder")]
fn user_file(w: &mut StaleWorld, name: String) {
    put_disk_in(&w.state, &old_mods_dir(), &name, b"manual-bytes");
}

#[given("a tracked file in the old folder was modified after the deploy")]
fn modified(w: &mut StaleWorld) {
    put_disk_in(&w.state, &old_mods_dir(), TRACKED[0], b"edited");
}

#[given("removing one of its files fails")]
fn removal_fails(w: &mut StaleWorld) {
    w.state.borrow_mut().fail_remove = vec![TRACKED[1].to_string()];
}

#[when("I inspect the deploy session")]
fn inspect(w: &mut StaleWorld) {
    w.snapshot();
    w.session = Some(service(&w.state).session(&mods_dir()).unwrap());
}

#[when("the user chooses to forget the old session")]
fn forget(w: &mut StaleWorld) {
    w.snapshot();
    service(&w.state)
        .forget_stale_session(&mods_dir())
        .expect("forget failed");
}

#[when("the user confirms cleaning the old folder")]
fn confirm_clean(w: &mut StaleWorld) {
    w.snapshot();
    let confirmed = w.recorded.clone();
    w.clean_result = Some(service(&w.state).clean_stale_session(&mods_dir(), &confirmed));
}

#[when("the user confirms cleaning a different folder")]
fn confirm_other(w: &mut StaleWorld) {
    w.snapshot();
    w.clean_result =
        Some(service(&w.state).clean_stale_session(&mods_dir(), &PathBuf::from("other/~mods")));
}

#[when("I deploy a profile into the authorized ~mods")]
fn deploy(w: &mut StaleWorld) {
    let mods = [mod_of(ID_A, &["A.pak"])];
    put_sources(&w.state, &mods);
    w.deploy_result = Some(service(&w.state).deploy(&mods_dir(), &mods));
}

#[then("it is reported as stale with the old folder and its tracked file count")]
fn reported_stale(w: &mut StaleWorld) {
    assert_eq!(
        w.session,
        Some(DeploySession::Stale {
            mods_dir: old_mods_dir(),
            tracked: 2
        })
    );
}

#[then("nothing is changed on disk or in the manifest")]
fn nothing_changed(w: &mut StaleWorld) {
    assert_eq!(Some(w.state.borrow().disk.clone()), w.disk_before);
    assert_eq!(Some(w.state.borrow().manifest.clone()), w.manifest_before);
    assert_eq!(w.state.borrow().manifest_saves, 0);
}

#[then("it is reported as vanished")]
fn reported_vanished(w: &mut StaleWorld) {
    assert_eq!(
        w.session,
        Some(DeploySession::Vanished {
            mods_dir: old_mods_dir()
        })
    );
}

#[then("it is reported as current")]
fn reported_current(w: &mut StaleWorld) {
    assert_eq!(w.session, Some(DeploySession::Current { tracked: 2 }));
}

#[then("the manifest is cleared")]
fn manifest_cleared(w: &mut StaleWorld) {
    assert_eq!(w.state.borrow().manifest, Deployed::default());
}

#[then("no file is deleted")]
fn no_file_deleted(w: &mut StaleWorld) {
    assert_eq!(Some(w.state.borrow().disk.clone()), w.disk_before);
}

#[then("only files proven to be ours are deleted from that folder")]
fn ours_deleted(w: &mut StaleWorld) {
    assert_eq!(w.clean_result.clone().unwrap().unwrap(), 2);
    assert!(TRACKED
        .iter()
        .all(|name| !disk_has_in(&w.state, &old_mods_dir(), name)));
}

#[then(expr = "{string} keeps the same bytes")]
fn same_bytes(w: &mut StaleWorld, name: String) {
    assert_eq!(
        w.state.borrow().disk[&(old_mods_dir(), name)],
        b"manual-bytes".to_vec()
    );
}

#[then("the modified file keeps its new bytes")]
fn modified_kept(w: &mut StaleWorld) {
    assert_eq!(
        w.state.borrow().disk[&(old_mods_dir(), TRACKED[0].to_string())],
        b"edited".to_vec()
    );
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
    assert_eq!(Some(w.state.borrow().manifest.clone()), w.manifest_before);
}

#[then("the manifest is cleared without an error")]
fn cleared_without_error(w: &mut StaleWorld) {
    assert_eq!(w.clean_result, Some(Ok(0)));
    manifest_cleared(w);
}

#[then("forgetting it is refused as not stale and the manifest is kept")]
fn forget_refused(w: &mut StaleWorld) {
    let result = service(&w.state).forget_stale_session(&mods_dir());
    assert!(matches!(result, Err(DeployError::SessionNotStale { .. })));
    manifest_kept(w);
}

#[then("deploying cleans it as before")]
fn deploy_cleans(w: &mut StaleWorld) {
    deploy(w);
    assert!(w.deploy_result.clone().unwrap().is_ok());
    assert!(TRACKED
        .iter()
        .all(|name| !disk_has_in(&w.state, &mods_dir(), name)));
}

#[then("the deploy succeeds")]
fn deploy_succeeds(w: &mut StaleWorld) {
    assert_eq!(
        w.deploy_result.clone().unwrap().unwrap(),
        vec!["000_aaaaaaaaaaaa_A.pak"]
    );
}

#[then("the cleanup reports it as incomplete")]
fn incomplete(w: &mut StaleWorld) {
    assert!(matches!(
        w.clean_result,
        Some(Err(DeployError::CleanIncomplete { .. }))
    ));
}

#[then("the failed entry stays tracked against the old folder")]
fn failed_tracked(w: &mut StaleWorld) {
    let manifest = w.state.borrow().manifest.clone();
    assert_eq!(manifest.mods_dir, old_mods_dir());
    assert_eq!(manifest_names(&manifest.files), vec![TRACKED[1]]);
}

/// Runs only `stale-manifest.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = StaleWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("stale-manifest.feature"))
        .await;
    !writer.execution_has_failed()
}
