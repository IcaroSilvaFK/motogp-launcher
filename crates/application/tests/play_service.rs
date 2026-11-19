//! Use-case tests of the Play orchestrator and startup recovery (issue #8):
//! fake game/profile/session ports over the real `DeployService` on in-memory
//! deploy ports. Nothing touches the real fs, Steam or processes.
#[path = "support/deploy_fakes.rs"]
pub mod deploy_fakes;
#[path = "support/play_fakes.rs"]
mod fakes;

use fakes::deploy::{disk_has, disk_names, put_disk, put_source, ID_A};
use fakes::*;
use motogp_application::{DeployError, PlayError, SessionError, SessionEvent, SessionState};
use motogp_domain::{Deployed, DeployedFile, LaunchError, WatchEvent};

fn ended(cleaned: usize) -> SessionEvent {
    SessionEvent {
        state: SessionState::Ended,
        cleaned,
    }
}

fn started() -> SessionEvent {
    SessionEvent {
        state: SessionState::Started,
        cleaned: 0,
    }
}

#[test]
fn play_without_game_does_not_deploy() {
    let rig = Rig::ready();
    rig.set_game(Err(PlayError::GameNotConfigured));

    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::GameNotConfigured);
    let disk = rig.disk.borrow();
    assert!(disk.dirs.is_empty(), "no ~mods directory may be created");
    assert!(disk.disk.is_empty());
    assert_eq!(disk.manifest_saves, 0);
    drop(disk);
    assert!(rig.planner.requested.lock().unwrap().is_empty());
    assert_eq!(rig.launches(), 0);

    // A future attempt remains possible once the game is configured.
    rig.set_game(Ok(game(Some(1))));
    rig.play.play("p1").expect("retry after configuring");
}

#[test]
fn play_without_app_id_does_not_deploy() {
    let rig = Rig::ready();
    rig.set_game(Ok(game(None)));

    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::Launch(LaunchError::MissingAppId));
    assert_eq!(
        error.to_string(),
        "game App ID not found; open the game from Steam once"
    );
    let disk = rig.disk.borrow();
    assert!(disk.dirs.is_empty());
    assert!(disk.disk.is_empty());
    assert_eq!(disk.manifest_saves, 0);
    drop(disk);
    assert_eq!(rig.launches(), 0);

    rig.set_game(Ok(game(Some(7))));
    rig.play.play("p1").expect("retry with an App ID");
}

#[test]
fn concurrent_play_is_rejected() {
    let rig = Rig::ready();
    rig.play.play("p1").expect("first play");
    let materialized = rig.materialized();

    // The session is still starting: the background work has not run yet.
    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::AlreadyRunning);
    assert_eq!(error.to_string(), "the game is already running");
    assert_eq!(rig.launches(), 1);
    assert_eq!(rig.materialized(), materialized);
}

#[test]
fn play_before_startup_recovery_is_rejected() {
    let rig = Rig::new();

    let error = rig.play.play("p1").unwrap_err();

    assert!(matches!(error, PlayError::RecoveryPending { .. }));
    assert_eq!(rig.launches(), 0);
    assert_eq!(rig.materialized(), 0);
}

#[test]
fn play_resolves_the_requested_profile_and_deploys_before_opening() {
    let rig = Rig::ready();

    rig.play.play("p9").expect("play");

    assert_eq!(*rig.planner.requested.lock().unwrap(), vec!["p9"]);
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    assert_eq!(rig.launches(), 1);
    assert!(
        rig.events().is_empty(),
        "nothing is published before tracking"
    );
}

#[test]
fn startup_recovers_previous_deployment() {
    let rig = Rig::new();
    let source = put_source(&rig.disk, ID_A, "A.pak", b"A.pak");
    put_disk(&rig.disk, DEPLOYED_A, b"A.pak");
    put_disk(&rig.disk, "manual.pak", b"user file");
    rig.disk.borrow_mut().manifest = Deployed {
        mods_dir: fakes::deploy::mods_dir(),
        files: vec![DeployedFile::new(DEPLOYED_A, source)],
        pending: vec![],
    };

    let cleaned = rig.play.recover_on_startup().expect("recovery");

    assert_eq!(cleaned, 1);
    assert_eq!(rig.play.recovered_count(), Some(1));
    assert_eq!(disk_names(&rig.disk), vec!["manual.pak"]);
    assert!(rig.disk.borrow().manifest.is_empty());
    rig.play
        .play("p1")
        .expect("play is accepted after recovery");
}

#[test]
fn startup_recovery_failure_is_visible_and_blocks_new_deploys() {
    let rig = Rig::new();
    let source = put_source(&rig.disk, ID_A, "A.pak", b"A.pak");
    put_disk(&rig.disk, DEPLOYED_A, b"A.pak");
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    rig.disk.borrow_mut().manifest = Deployed {
        mods_dir: fakes::deploy::mods_dir(),
        files: vec![DeployedFile::new(DEPLOYED_A, source)],
        pending: vec![],
    };

    let error = rig.play.recover_on_startup().unwrap_err();

    assert!(matches!(
        error,
        PlayError::Deploy(DeployError::CleanIncomplete { .. })
    ));
    assert_eq!(rig.errors().len(), 1);
    assert_eq!(
        rig.disk.borrow().manifest.files.len(),
        1,
        "still recoverable"
    );
    let materialized = rig.materialized();

    let blocked = rig.play.play("p1").unwrap_err();

    assert!(matches!(blocked, PlayError::RecoveryPending { .. }));
    assert_eq!(rig.launches(), 0);
    assert_eq!(rig.materialized(), materialized);

    // Once the file can be removed again, the next Play resolves the block.
    rig.disk.borrow_mut().fail_remove.clear();
    rig.play.play("p1").expect("recovered on retry");
    assert!(disk_has(&rig.disk, DEPLOYED_A));
}

#[test]
fn ended_session_cleans_and_emits_count() {
    let rig = Rig::ready();
    rig.play.play("p1").expect("play");
    assert!(disk_has(&rig.disk, DEPLOYED_A));

    rig.runner.run_pending();

    assert_eq!(rig.events(), vec![started(), ended(1)]);
    assert!(disk_names(&rig.disk).is_empty());
    assert!(rig.disk.borrow().manifest.is_empty());
    assert!(rig.errors().is_empty());
    rig.play
        .play("p1")
        .expect("a new session after the cleanup");
}

#[test]
fn never_started_session_cleans_and_releases_lock() {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![WatchEvent::NeverStarted];
    rig.play.play("p1").expect("play");

    rig.runner.run_pending();

    assert_eq!(
        rig.events(),
        vec![SessionEvent {
            state: SessionState::NeverStarted,
            cleaned: 1
        }]
    );
    assert!(disk_names(&rig.disk).is_empty());
    rig.play.play("p1").expect("the lock was released");
}

#[test]
fn started_event_does_not_clean() {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![WatchEvent::Started];
    *rig.runtime.outcome.lock().unwrap() = Ok(motogp_application::TrackOutcome::Cancelled);
    rig.play.play("p1").expect("play");

    rig.runner.run_pending();

    assert_eq!(rig.events(), vec![started()]);
    assert!(disk_has(&rig.disk, DEPLOYED_A), "files stay deployed");
    assert_eq!(
        rig.play.play("p1").unwrap_err(),
        PlayError::AlreadyRunning,
        "the session is still active"
    );
}

#[test]
fn opener_failure_rolls_back_deployment() {
    let rig = Rig::ready();
    let failure = SessionError::Opener(motogp_application::AppError::Read {
        location: "steam://rungameid/1".into(),
        detail: "no handler".into(),
    });
    *rig.runtime.launch_result.lock().unwrap() = Err(failure.clone());

    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::Session(failure));
    assert!(
        disk_names(&rig.disk).is_empty(),
        "only this session's files were removed"
    );
    assert!(rig.disk.borrow().manifest.is_empty());
    assert!(rig.events().is_empty());

    *rig.runtime.launch_result.lock().unwrap() = Ok(());
    rig.play.play("p1").expect("the lock was released");
}

#[test]
fn opener_failure_keeps_manual_files_untouched() {
    let rig = Rig::ready();
    put_disk(&rig.disk, "manual.pak", b"user file");
    *rig.runtime.launch_result.lock().unwrap() =
        Err(SessionError::Launch(LaunchError::MissingAppId));

    let _ = rig.play.play("p1");

    assert_eq!(disk_names(&rig.disk), vec!["manual.pak"]);
}

#[test]
fn failed_compensation_is_reported_and_blocks_new_deploys() {
    let rig = Rig::ready();
    *rig.runtime.launch_result.lock().unwrap() =
        Err(SessionError::Launch(LaunchError::MissingAppId));
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];

    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(
        error,
        PlayError::Session(SessionError::Launch(LaunchError::MissingAppId))
    );
    assert_eq!(rig.errors().len(), 1);
    assert_eq!(
        rig.disk.borrow().manifest.files.len(),
        1,
        "kept for recovery"
    );
    assert!(matches!(
        rig.play.play("p1").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));
}

#[test]
fn cleanup_failure_keeps_recovery_pending() {
    let rig = Rig::ready();
    rig.play.play("p1").expect("play");
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];

    rig.runner.run_pending();

    assert_eq!(
        rig.events(),
        vec![started()],
        "no ended event on a failed cleanup"
    );
    assert_eq!(rig.errors().len(), 1);
    assert!(rig.errors()[0].contains("cleanup incomplete"));
    assert_eq!(rig.disk.borrow().manifest.files.len(), 1, "manifest kept");
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    let materialized = rig.materialized();

    let blocked = rig.play.play("p1").unwrap_err();

    assert!(matches!(blocked, PlayError::RecoveryPending { .. }));
    assert_eq!(rig.materialized(), materialized, "no unsafe new deploy");

    rig.disk.borrow_mut().fail_remove.clear();
    rig.play
        .play("p1")
        .expect("retry succeeds once cleanup works");
}

#[test]
fn tracking_failure_is_reported_without_a_false_success() {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![];
    *rig.runtime.outcome.lock().unwrap() = Err(motogp_application::AppError::Read {
        location: "processes".into(),
        detail: "denied".into(),
    });
    rig.play.play("p1").expect("play");

    rig.runner.run_pending();

    assert!(rig.events().is_empty());
    assert_eq!(rig.errors().len(), 1);
    assert!(
        disk_has(&rig.disk, DEPLOYED_A),
        "files stay for a later cleanup"
    );
}
