//! Steps for `tests/features/restore-game.feature` (issue #150), over the real
//! `PlayService` and `DeployService` with in-memory ports.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{PlayError, RecoveryState};
use motogp_domain::Deployed;

use crate::play_fakes::deploy::{disk_has, manifest_names, mods_dir, stale_manifest};
use crate::play_fakes::Rig;

const DEPLOYED: [&str; 2] = ["000_A.pak", "001_B.pak"];

#[derive(Default, World)]
struct RestoreWorld {
    rig: Option<Rig>,
    result: Option<Result<usize, PlayError>>,
}

impl std::fmt::Debug for RestoreWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RestoreWorld")
    }
}

impl RestoreWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }
}

#[given("a configured game with 2 deployed files")]
fn configured(w: &mut RestoreWorld) {
    let rig = Rig::ready();
    stale_manifest(&rig.disk, &mods_dir(), &DEPLOYED);
    w.rig = Some(rig);
}

#[given("the startup cleanup failed")]
fn startup_failed(w: &mut RestoreWorld) {
    let rig = Rig::new();
    stale_manifest(&rig.disk, &mods_dir(), &DEPLOYED);
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED[0].to_string()];
    assert!(rig.play.recover_on_startup().is_err());
    // The cause went away (for example the file was unlocked).
    rig.disk.borrow_mut().fail_remove.clear();
    w.rig = Some(rig);
}

#[given("a game session is active")]
fn session_active(w: &mut RestoreWorld) {
    let rig = w.rig();
    // Park the manifest while Play deploys, then restore it.
    let manifest = rig.disk.borrow().manifest.clone();
    rig.disk.borrow_mut().manifest = Deployed::default();
    rig.play.play("p").expect("play");
    rig.disk.borrow_mut().manifest = manifest;
}

#[given("the previous game is still running")]
fn game_running(w: &mut RestoreWorld) {
    let rig = Rig::new();
    stale_manifest(&rig.disk, &mods_dir(), &DEPLOYED);
    *rig.runtime.running.lock().unwrap() = Ok(true);
    rig.play.recover_on_startup().expect("startup recovery");
    w.rig = Some(rig);
}

#[when("the user restores the original game files")]
fn restore(w: &mut RestoreWorld) {
    w.result = Some(w.rig().play.restore_game_files());
}

#[then("2 files are removed and the manifest is empty")]
fn removed(w: &mut RestoreWorld) {
    assert_eq!(w.result, Some(Ok(2)));
    assert_eq!(w.rig().disk.borrow().manifest, Deployed::default());
    assert!(DEPLOYED.iter().all(|name| !disk_has(&w.rig().disk, name)));
}

#[then("the recovery status is recovered")]
fn status_recovered(w: &mut RestoreWorld) {
    assert_eq!(
        w.rig().play.recovery_status().state,
        RecoveryState::Recovered
    );
}

#[then("Play succeeds")]
fn play_succeeds(w: &mut RestoreWorld) {
    assert_eq!(w.rig().play.play("p"), Ok(()));
    assert_eq!(w.rig().launches(), 1);
}

#[then("it is refused because a session is running")]
fn refused_session(w: &mut RestoreWorld) {
    assert_eq!(w.result, Some(Err(PlayError::AlreadyRunning)));
}

#[then("it is refused because the game is still running")]
fn refused_game(w: &mut RestoreWorld) {
    assert_eq!(w.result, Some(Err(PlayError::GameStillRunning)));
}

#[then("nothing is removed")]
fn nothing_removed(w: &mut RestoreWorld) {
    assert!(DEPLOYED.iter().all(|name| disk_has(&w.rig().disk, name)));
    assert_eq!(
        manifest_names(&w.rig().disk.borrow().manifest.files),
        DEPLOYED
    );
}

/// Runs only `restore-game.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = RestoreWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("restore-game.feature"))
        .await;
    !writer.execution_has_failed()
}
