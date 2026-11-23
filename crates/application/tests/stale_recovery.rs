//! Stale-session recovery through `PlayService` (issue #34): the same
//! `DeployService` rules as #23, plus the session lock, the running-game guard
//! and the recovery state (a stale manifest at startup blocks Play).
#[path = "support/deploy_fakes.rs"]
pub mod deploy_fakes;
#[path = "support/play_fakes.rs"]
mod fakes;

use fakes::deploy::{disk_has_in, manifest_names, mods_dir, old_mods_dir, stale_manifest};
use fakes::*;
use motogp_application::{DeployError, DeploySession, PlayError, RecoveryState};
use motogp_domain::Deployed;

const OLD: [&str; 2] = ["000_old_A.pak", "001_old_B.pak"];

fn stale(rig: &Rig) {
    stale_manifest(&rig.disk, &old_mods_dir(), &OLD);
}

fn blocked_at_startup() -> Rig {
    let rig = Rig::new();
    stale(&rig);
    assert!(rig.play.recover_on_startup().is_err());
    rig
}

#[test]
fn deploy_session_reports_none_current_stale_and_vanished() {
    let rig = Rig::ready();
    assert_eq!(rig.play.deploy_session().unwrap(), DeploySession::None);

    stale_manifest(&rig.disk, &mods_dir(), &OLD);
    assert_eq!(
        rig.play.deploy_session().unwrap(),
        DeploySession::Current { tracked: 2 }
    );

    stale(&rig);
    assert_eq!(
        rig.play.deploy_session().unwrap(),
        DeploySession::Stale {
            mods_dir: old_mods_dir(),
            tracked: 2
        }
    );

    rig.disk.borrow_mut().disk.clear();
    rig.disk.borrow_mut().missing_dirs.push(old_mods_dir());
    assert_eq!(
        rig.play.deploy_session().unwrap(),
        DeploySession::Vanished {
            mods_dir: old_mods_dir()
        }
    );
}

#[test]
fn deploy_session_requires_a_configured_game() {
    let rig = Rig::ready();
    rig.set_game(Err(PlayError::GameNotConfigured));
    assert_eq!(
        rig.play.deploy_session().unwrap_err(),
        PlayError::GameNotConfigured
    );
}

#[test]
fn forgetting_a_stale_session_empties_the_manifest_and_keeps_the_files() {
    let rig = Rig::ready();
    stale(&rig);

    rig.play.forget_stale_session().unwrap();

    assert_eq!(rig.disk.borrow().manifest, Deployed::default());
    assert!(OLD
        .iter()
        .all(|name| disk_has_in(&rig.disk, &old_mods_dir(), name)));
    assert_eq!(rig.play.deploy_session().unwrap(), DeploySession::None);
}

#[test]
fn play_succeeds_after_forgetting_a_stale_session() {
    let rig = Rig::ready();
    stale(&rig);
    rig.play.forget_stale_session().unwrap();

    rig.play.play("p").unwrap();

    assert_eq!(rig.launches(), 1);
}

#[test]
fn cleaning_with_the_confirmed_folder_removes_only_owned_files() {
    let rig = Rig::ready();
    stale(&rig);
    deploy_fakes::put_disk_in(&rig.disk, &old_mods_dir(), "meu_mod.pak", b"manual");

    let cleaned = rig.play.clean_stale_session(&old_mods_dir()).unwrap();

    assert_eq!(cleaned, 2);
    assert!(OLD
        .iter()
        .all(|name| !disk_has_in(&rig.disk, &old_mods_dir(), name)));
    assert!(disk_has_in(&rig.disk, &old_mods_dir(), "meu_mod.pak"));
    assert_eq!(rig.disk.borrow().manifest, Deployed::default());
}

#[test]
fn cleaning_with_a_different_folder_is_refused_and_removes_nothing() {
    let rig = Rig::ready();
    stale(&rig);

    let error = rig
        .play
        .clean_stale_session(std::path::Path::new("other/~mods"))
        .unwrap_err();

    assert!(matches!(
        error,
        PlayError::Deploy(DeployError::UnauthorizedDirectory { .. })
    ));
    assert!(OLD
        .iter()
        .all(|name| disk_has_in(&rig.disk, &old_mods_dir(), name)));
    assert_eq!(manifest_names(&rig.disk.borrow().manifest.files), OLD);
}

#[test]
fn forgetting_the_current_session_is_refused_and_keeps_the_manifest() {
    let rig = Rig::ready();
    stale_manifest(&rig.disk, &mods_dir(), &OLD);

    let error = rig.play.forget_stale_session().unwrap_err();

    assert!(matches!(
        error,
        PlayError::Deploy(DeployError::SessionNotStale { .. })
    ));
    assert_eq!(manifest_names(&rig.disk.borrow().manifest.files), OLD);
}

#[test]
fn recovery_is_refused_while_a_session_holds_the_lock() {
    let rig = Rig::ready();
    rig.play.play("p").unwrap();
    stale(&rig);

    assert_eq!(
        rig.play.forget_stale_session().unwrap_err(),
        PlayError::AlreadyRunning
    );
    assert_eq!(
        rig.play.clean_stale_session(&old_mods_dir()).unwrap_err(),
        PlayError::AlreadyRunning
    );
    assert_eq!(manifest_names(&rig.disk.borrow().manifest.files), OLD);
}

#[test]
fn recovery_is_refused_while_the_previous_game_is_still_running() {
    let rig = Rig::new();
    stale_manifest(&rig.disk, &mods_dir(), &OLD);
    *rig.runtime.running.lock().unwrap() = Ok(true);
    rig.play.recover_on_startup().unwrap();
    stale(&rig);

    assert_eq!(
        rig.play.forget_stale_session().unwrap_err(),
        PlayError::GameStillRunning
    );
    assert_eq!(
        rig.play.clean_stale_session(&old_mods_dir()).unwrap_err(),
        PlayError::GameStillRunning
    );
    assert_eq!(manifest_names(&rig.disk.borrow().manifest.files), OLD);
}

#[test]
fn recovery_releases_the_lock_and_does_not_block_play() {
    let rig = Rig::ready();
    stale(&rig);
    rig.play.forget_stale_session().unwrap();
    stale(&rig);
    rig.play.clean_stale_session(&old_mods_dir()).unwrap();

    rig.play.play("p").unwrap();
    assert_eq!(rig.launches(), 1);
}

#[test]
fn forgetting_after_a_stale_startup_failure_unblocks_play() {
    let rig = blocked_at_startup();
    assert_eq!(rig.play.recovery_status().state, RecoveryState::Failed);
    assert!(matches!(
        rig.play.play("p").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));

    rig.play.forget_stale_session().unwrap();

    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Recovered);
    assert_eq!(status.error, None);
    assert_eq!(status.cleaned_count, 0);
    rig.play.play("p").unwrap();
    assert_eq!(rig.launches(), 1);
}

#[test]
fn cleaning_after_a_stale_startup_failure_unblocks_play_and_reports_the_count() {
    let rig = blocked_at_startup();

    rig.play.clean_stale_session(&old_mods_dir()).unwrap();

    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Recovered);
    assert_eq!(status.cleaned_count, 2);
    rig.play.play("p").unwrap();
}

#[test]
fn a_refused_recovery_keeps_the_blocked_state() {
    let rig = blocked_at_startup();

    assert!(rig
        .play
        .clean_stale_session(std::path::Path::new("other/~mods"))
        .is_err());

    assert_eq!(rig.play.recovery_status().state, RecoveryState::Failed);
    assert!(matches!(
        rig.play.play("p").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));
}

#[test]
fn recovery_requires_a_configured_game() {
    let rig = Rig::ready();
    stale(&rig);
    rig.set_game(Err(PlayError::GameNotConfigured));

    assert_eq!(
        rig.play.forget_stale_session().unwrap_err(),
        PlayError::GameNotConfigured
    );
    assert_eq!(
        rig.play.clean_stale_session(&old_mods_dir()).unwrap_err(),
        PlayError::GameNotConfigured
    );
    assert_eq!(manifest_names(&rig.disk.borrow().manifest.files), OLD);
}
