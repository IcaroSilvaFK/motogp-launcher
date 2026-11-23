//! Steps for `tests/features/stale-session-recovery.feature` (issue #34), over
//! the real `PlayService` and `DeployService` with in-memory ports.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{DeployError, DeploySession, PlayError, RecoveryState};
use motogp_domain::Deployed;
use std::path::PathBuf;

use crate::play_fakes::deploy::{
    disk_has_in, manifest_names, mods_dir, old_mods_dir, put_disk_in, stale_manifest,
};
use crate::play_fakes::Rig;

const OLD: [&str; 2] = ["000_old_A.pak", "001_old_B.pak"];

#[derive(Default, World)]
struct RecoveryWorld {
    rig: Option<Rig>,
    forget: Option<Result<(), PlayError>>,
    clean: Option<Result<usize, PlayError>>,
}

impl std::fmt::Debug for RecoveryWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecoveryWorld")
    }
}

impl RecoveryWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }

    /// The error of whichever recovery action ran.
    fn error(&self) -> PlayError {
        match (&self.forget, &self.clean) {
            (Some(Err(error)), _) => error.clone(),
            (_, Some(Err(error))) => error.clone(),
            _ => panic!("the recovery did not fail"),
        }
    }
}

#[given("a configured game and a manifest that points to a previous game install")]
fn configured(w: &mut RecoveryWorld) {
    let rig = Rig::ready();
    stale_manifest(&rig.disk, &old_mods_dir(), &OLD);
    put_disk_in(&rig.disk, &old_mods_dir(), "meu_mod.pak", b"manual");
    w.rig = Some(rig);
}

#[given("the manifest points to the current game install")]
fn current(w: &mut RecoveryWorld) {
    stale_manifest(&w.rig().disk, &mods_dir(), &OLD);
}

#[given("the old folder no longer exists")]
fn vanished(w: &mut RecoveryWorld) {
    w.rig().disk.borrow_mut().disk.clear();
    w.rig().disk.borrow_mut().missing_dirs.push(old_mods_dir());
}

#[given("a game session is active")]
fn session_active(w: &mut RecoveryWorld) {
    let rig = w.rig();
    // Park the manifest while Play deploys, then restore it.
    let manifest = rig.disk.borrow().manifest.clone();
    rig.disk.borrow_mut().manifest = Deployed::default();
    rig.play.play("p").expect("play");
    rig.disk.borrow_mut().manifest = manifest;
}

#[given("the previous game is still running")]
fn game_running(w: &mut RecoveryWorld) {
    let rig = Rig::new();
    stale_manifest(&rig.disk, &mods_dir(), &OLD);
    *rig.runtime.running.lock().unwrap() = Ok(true);
    rig.play.recover_on_startup().expect("startup recovery");
    stale_manifest(&rig.disk, &old_mods_dir(), &OLD);
    w.rig = Some(rig);
}

#[given("the startup recovery failed because of the stale manifest")]
fn startup_failed(w: &mut RecoveryWorld) {
    let rig = Rig::new();
    stale_manifest(&rig.disk, &old_mods_dir(), &OLD);
    assert!(rig.play.recover_on_startup().is_err());
    w.rig = Some(rig);
}

#[given("no game is configured")]
fn no_game(w: &mut RecoveryWorld) {
    w.rig().set_game(Err(PlayError::GameNotConfigured));
}

#[when("the user forgets the old session")]
fn forget(w: &mut RecoveryWorld) {
    w.forget = Some(w.rig().play.forget_stale_session());
}

#[when("the user confirms cleaning the old folder")]
fn confirm_clean(w: &mut RecoveryWorld) {
    w.clean = Some(w.rig().play.clean_stale_session(&old_mods_dir()));
}

#[when("the user confirms cleaning a different folder")]
fn confirm_other(w: &mut RecoveryWorld) {
    w.clean = Some(
        w.rig()
            .play
            .clean_stale_session(&PathBuf::from("other/~mods")),
    );
}

#[then("the manifest is empty")]
fn manifest_empty(w: &mut RecoveryWorld) {
    assert_eq!(w.rig().disk.borrow().manifest, Deployed::default());
}

#[then("the old files are untouched")]
fn old_untouched(w: &mut RecoveryWorld) {
    assert!(OLD
        .iter()
        .all(|name| disk_has_in(&w.rig().disk, &old_mods_dir(), name)));
}

#[then("only the files of the manifest are removed from the old folder")]
fn only_owned_removed(w: &mut RecoveryWorld) {
    assert_eq!(w.clean, Some(Ok(2)));
    assert!(OLD
        .iter()
        .all(|name| !disk_has_in(&w.rig().disk, &old_mods_dir(), name)));
    assert!(disk_has_in(&w.rig().disk, &old_mods_dir(), "meu_mod.pak"));
}

#[then("the cleanup is refused as unauthorized")]
fn unauthorized(w: &mut RecoveryWorld) {
    assert!(matches!(
        w.error(),
        PlayError::Deploy(DeployError::UnauthorizedDirectory { .. })
    ));
}

#[then("nothing is removed")]
fn nothing_removed(w: &mut RecoveryWorld) {
    old_untouched(w);
    manifest_kept(w);
}

#[then("the recovery is refused as not stale")]
fn not_stale(w: &mut RecoveryWorld) {
    assert!(matches!(
        w.error(),
        PlayError::Deploy(DeployError::SessionNotStale { .. })
    ));
}

#[then("the manifest is kept")]
fn manifest_kept(w: &mut RecoveryWorld) {
    assert_eq!(manifest_names(&w.rig().disk.borrow().manifest.files), OLD);
}

#[then("the deploy session is vanished")]
fn is_vanished(w: &mut RecoveryWorld) {
    assert_eq!(
        w.rig().play.deploy_session().unwrap(),
        DeploySession::Vanished {
            mods_dir: old_mods_dir()
        }
    );
}

#[then("the recovery is refused because a session is running")]
fn refused_running_session(w: &mut RecoveryWorld) {
    assert_eq!(w.error(), PlayError::AlreadyRunning);
}

#[then("the recovery is refused because the game is running")]
fn refused_running_game(w: &mut RecoveryWorld) {
    assert_eq!(w.error(), PlayError::GameStillRunning);
}

#[then("the recovery is refused because no game is configured")]
fn refused_no_game(w: &mut RecoveryWorld) {
    assert_eq!(w.error(), PlayError::GameNotConfigured);
}

#[then("the recovery status is recovered")]
fn status_recovered(w: &mut RecoveryWorld) {
    assert_eq!(
        w.rig().play.recovery_status().state,
        RecoveryState::Recovered
    );
}

#[then("Play succeeds")]
fn play_succeeds(w: &mut RecoveryWorld) {
    assert_eq!(w.rig().play.play("p"), Ok(()));
    assert_eq!(w.rig().launches(), 1);
}

/// Runs only `stale-session-recovery.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = RecoveryWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("stale-session-recovery.feature"))
        .await;
    !writer.execution_has_failed()
}
