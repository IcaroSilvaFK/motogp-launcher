//! Play orchestrator hardening (issue #8 review): a still-running game is
//! never cleaned or redeployed over, the session lock is panic-safe, a failed
//! worker spawn is recoverable and the startup recovery stays retrievable.
#[path = "support/deploy_fakes.rs"]
pub mod deploy_fakes;
#[path = "support/play_fakes.rs"]
mod fakes;

use fakes::deploy::{disk_has, disk_names, put_disk, put_source, ID_A};
use fakes::*;
use motogp_application::{
    AppError, PlayError, RecoveryState, SessionEvent, SessionState, TrackOutcome,
};
use motogp_domain::{Deployed, DeployedFile, WatchEvent};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::Ordering;

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

fn leave_deployed(rig: &Rig) {
    let source = put_source(&rig.disk, ID_A, "A.pak", b"A.pak");
    put_disk(&rig.disk, DEPLOYED_A, b"A.pak");
    rig.disk.borrow_mut().manifest = Deployed {
        mods_dir: fakes::deploy::mods_dir(),
        files: vec![DeployedFile::new(DEPLOYED_A, source)],
        pending: vec![],
    };
}

fn set_running(rig: &Rig, running: Result<bool, AppError>) {
    *rig.runtime.running.lock().unwrap() = running;
}

fn denied() -> AppError {
    AppError::Read {
        location: "processes".into(),
        detail: "denied".into(),
    }
}

#[test]
fn restart_with_the_game_running_keeps_files_and_resumes_tracking() {
    let rig = Rig::new();
    leave_deployed(&rig);
    set_running(&rig, Ok(true));

    let cleaned = rig.play.recover_on_startup().expect("recovery");

    assert_eq!(cleaned, 0, "nothing is cleaned while the game runs");
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    assert_eq!(rig.disk.borrow().manifest.files.len(), 1);
    assert_eq!(
        rig.play.recovery_status().state,
        RecoveryState::GameRunning
    );
    assert_eq!(
        rig.runner.pending(),
        1,
        "tracking resumed in the background"
    );
    assert_eq!(rig.play.play("p1").unwrap_err(), PlayError::GameStillRunning);

    // The game ends: the deployment is cleaned and Play works again.
    rig.runner.run_pending();

    assert!(disk_names(&rig.disk).is_empty());
    assert_eq!(rig.events(), vec![started(), ended(1)]);
    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Recovered);
    assert_eq!(status.cleaned_count, 1);
    set_running(&rig, Ok(false));
    rig.play.play("p1").expect("play after the game ended");
}

#[test]
fn startup_inspection_failure_blocks_and_deletes_nothing() {
    let rig = Rig::new();
    leave_deployed(&rig);
    set_running(&rig, Err(denied()));

    let error = rig.play.recover_on_startup().unwrap_err();

    assert!(matches!(error, PlayError::ProcessInspection { .. }));
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    assert_eq!(rig.disk.borrow().manifest.files.len(), 1);
    assert_eq!(rig.errors().len(), 1);
    assert!(matches!(
        rig.play.play("p1").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));
    assert_eq!(rig.launches(), 0);

    set_running(&rig, Ok(false));
    rig.play
        .play("p1")
        .expect("cleaned once the inspection works");
}

#[test]
fn play_is_refused_while_the_game_is_running() {
    let rig = Rig::ready();
    set_running(&rig, Ok(true));

    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::GameStillRunning);
    assert_eq!(rig.materialized(), 0);
    assert_eq!(rig.launches(), 0);
    assert!(rig.lock.try_acquire(), "the lock was released");
}

#[test]
fn play_inspection_failure_blocks_deployment() {
    let rig = Rig::ready();
    set_running(&rig, Err(denied()));

    let error = rig.play.play("p1").unwrap_err();

    assert!(matches!(error, PlayError::ProcessInspection { .. }));
    assert_eq!(rig.materialized(), 0);
    assert_eq!(rig.launches(), 0);
}

#[test]
fn tracking_failure_with_the_game_running_never_redeploys_until_it_ends() {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![];
    *rig.runtime.outcome.lock().unwrap() = Err(denied());
    rig.play.play("p1").expect("play");
    rig.runner.run_pending();
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    let materialized = rig.materialized();

    // The game still runs: no clean, no redeploy, tracking is resumed.
    set_running(&rig, Ok(true));
    *rig.runtime.events.lock().unwrap() = vec![WatchEvent::Started, WatchEvent::Ended];
    *rig.runtime.outcome.lock().unwrap() = Ok(TrackOutcome::Finished);
    let error = rig.play.play("p1").unwrap_err();

    assert_eq!(error, PlayError::GameStillRunning);
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    assert_eq!(rig.materialized(), materialized);
    assert_eq!(rig.runner.pending(), 1);

    rig.runner.run_pending();

    assert!(disk_names(&rig.disk).is_empty(), "cleaned after it ended");
    set_running(&rig, Ok(false));
    rig.play.play("p1").expect("play after termination");
}

#[test]
fn tracking_failure_then_inspection_failure_deletes_nothing() {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![];
    *rig.runtime.outcome.lock().unwrap() = Err(denied());
    rig.play.play("p1").expect("play");
    rig.runner.run_pending();
    set_running(&rig, Err(denied()));

    let error = rig.play.play("p1").unwrap_err();

    assert!(matches!(error, PlayError::ProcessInspection { .. }));
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    assert_eq!(rig.disk.borrow().manifest.files.len(), 1);
}

/// The lock is free again and the next Play first retries the cleanup (which
/// we make fail, so the block is observable as `RecoveryPending`).
fn assert_recovery_pending(rig: &Rig) {
    assert!(rig.lock.try_acquire(), "the lock was released");
    rig.lock.release();
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    assert!(matches!(
        rig.play.play("p1").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));
}

#[test]
fn panic_while_tracking_releases_the_lock_and_marks_recovery_pending() {
    let rig = Rig::ready();
    rig.runtime.track_panics.store(true, Ordering::SeqCst);
    rig.play.play("p1").expect("play");

    let outcome = catch_unwind(AssertUnwindSafe(|| rig.runner.run_pending()));

    assert!(outcome.is_err());
    assert!(disk_has(&rig.disk, DEPLOYED_A), "files stay deployed");
    assert_eq!(rig.errors().len(), 1);
    assert_recovery_pending(&rig);
}

#[test]
fn panic_while_preparing_releases_the_lock_and_marks_recovery_pending() {
    let rig = Rig::ready();
    rig.planner.panics.store(true, Ordering::SeqCst);

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = rig.play.play("p1");
    }));

    assert!(outcome.is_err());
    assert_eq!(rig.errors().len(), 1);
    assert!(rig.lock.try_acquire(), "the lock was released on unwind");
    rig.lock.release();
    rig.planner.panics.store(false, Ordering::SeqCst);
    rig.play.play("p1").expect("recovers after the panic");
}

#[test]
fn spawn_failure_keeps_files_marks_recovery_pending_and_reports_an_error() {
    let rig = Rig::ready();
    rig.runner.fail_spawn.store(true, Ordering::SeqCst);

    let error = rig.play.play("p1").unwrap_err();

    assert!(matches!(error, PlayError::SessionThread { .. }));
    assert!(disk_has(&rig.disk, DEPLOYED_A), "the game was opened");
    assert_eq!(rig.disk.borrow().manifest.files.len(), 1);
    assert_recovery_pending(&rig);
}

#[test]
fn recovery_status_keeps_a_startup_failure_retrievable() {
    let rig = Rig::new();
    assert_eq!(rig.play.recovery_status().state, RecoveryState::NotRun);
    leave_deployed(&rig);
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];

    let _ = rig.play.recover_on_startup().unwrap_err();

    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Failed);
    assert!(status.error.unwrap().contains("cleanup incomplete"));
    assert_eq!(status.cleaned_count, 0);

    rig.disk.borrow_mut().fail_remove.clear();
    rig.play.play("p1").expect("retry cleans");
    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Recovered);
    assert_eq!(status.error, None);
}

#[test]
fn recovery_status_reports_the_cleaned_count() {
    let rig = Rig::new();
    leave_deployed(&rig);

    rig.play.recover_on_startup().expect("recovery");

    let status = rig.play.recovery_status();
    assert_eq!(status.state, RecoveryState::Recovered);
    assert_eq!(status.cleaned_count, 1);
    assert_eq!(status.error, None);
}
