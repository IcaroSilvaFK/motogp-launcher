//! Steps for `tests/features/session-control.feature` (issue #75), fake ports
//! only. The live-session scenarios run the worker on a real thread against a
//! fake game that stays open until it is stopped.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, PlayError, SessionPhase, SessionState};
use std::sync::atomic::Ordering;

use crate::play_fakes;
use play_fakes::deploy::{disk_has, disk_names};
use play_fakes::{Rig, DEPLOYED_A};

#[derive(Default, World)]
struct ControlWorld {
    rig: Option<Rig>,
    error: Option<PlayError>,
}

impl std::fmt::Debug for ControlWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ControlWorld")
    }
}

impl ControlWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }
}

fn phase_name(phase: SessionPhase) -> &'static str {
    match phase {
        SessionPhase::Idle => "idle",
        SessionPhase::Starting => "starting",
        SessionPhase::Running => "running",
        SessionPhase::Stopping => "stopping",
        SessionPhase::CleaningUp => "cleaning-up",
    }
}

fn state_name(state: SessionState) -> &'static str {
    match state {
        SessionState::Started => "started",
        SessionState::CleaningUp => "cleaning-up",
        SessionState::Ended => "ended",
        SessionState::NeverStarted => "never-started",
    }
}

fn clock_ms(hours: u64, minutes: u64) -> u64 {
    (hours * 60 + minutes) * 60_000
}

fn running_session(files: &[&str]) -> Rig {
    let rig = Rig::live(files);
    rig.play.play("p1").expect("play");
    rig.wait_until("the game is seen", |r| {
        r.play.current_session().phase == SessionPhase::Running
    });
    rig
}

#[given("startup recovery has run and nothing is deployed")]
fn recovered(w: &mut ControlWorld) {
    w.rig = Some(Rig::ready());
}

#[given("no session is active")]
fn no_session(w: &mut ControlWorld) {
    w.rig = Some(Rig::ready());
}

#[given(regex = r"^a session was started and the game process appeared at (\d+):(\d+)$")]
fn started_at(w: &mut ControlWorld, hours: u64, minutes: u64) {
    let rig = Rig::live(&["A.pak"]);
    rig.runtime
        .now_ms
        .store(clock_ms(hours, minutes), Ordering::SeqCst);
    rig.play.play("p1").expect("play");
    rig.wait_until("the game is seen", |r| {
        r.play.current_session().phase == SessionPhase::Running
    });
    w.rig = Some(rig);
}

#[given("a running session")]
fn running(w: &mut ControlWorld) {
    w.rig = Some(running_session(&["A.pak"]));
}

#[given("a running session with 3 deployed files")]
fn running_with_files(w: &mut ControlWorld) {
    w.rig = Some(running_session(&["A.pak", "B.pak", "C.pak"]));
}

#[given("a session that has not seen the game yet")]
fn not_seen(w: &mut ControlWorld) {
    let rig = Rig::live(&["A.pak"]);
    rig.runtime.events.lock().unwrap().clear();
    rig.play.play("p1").expect("play");
    assert_eq!(rig.play.current_session().phase, SessionPhase::Starting);
    w.rig = Some(rig);
}

#[given("the game processes cannot be terminated")]
fn cannot_terminate(w: &mut ControlWorld) {
    *w.rig().runtime.terminate_result.lock().unwrap() = Err(AppError::Read {
        location: "processes".into(),
        detail: "access denied".into(),
    });
}

#[when("the game exits")]
fn game_exits(w: &mut ControlWorld) {
    w.rig().runtime.exit.store(true, Ordering::SeqCst);
}

#[when("the user stops the session")]
fn stop(w: &mut ControlWorld) {
    w.error = w.rig().play.stop_session().err();
}

#[then(regex = r#"^the current session phase is "([a-z-]+)"$"#)]
fn phase_is(w: &mut ControlWorld, phase: String) {
    w.rig().wait_until("the phase is reached", |r| {
        phase_name(r.play.current_session().phase) == phase
    });
}

#[then(regex = r#"^the current session phase is "running" since (\d+):(\d+)$"#)]
fn running_since(w: &mut ControlWorld, hours: u64, minutes: u64) {
    let snapshot = w.rig().play.current_session();
    assert_eq!(snapshot.phase, SessionPhase::Running);
    assert_eq!(snapshot.started_at_ms, Some(clock_ms(hours, minutes)));
}

#[then(regex = r#"^the events are "started", "cleaning-up" and "ended" in that order$"#)]
fn events_in_order(w: &mut ControlWorld) {
    w.rig()
        .wait_until("the session ended", |r| r.events().len() == 3);
    let names: Vec<&str> = w
        .rig()
        .events()
        .iter()
        .map(|e| state_name(e.state))
        .collect();
    assert_eq!(names, vec!["started", "cleaning-up", "ended"]);
}

#[then("the game processes inside the install root are terminated")]
fn terminated(w: &mut ControlWorld) {
    assert!(w.error.is_none(), "stop failed: {:?}", w.error);
    assert_eq!(w.rig().runtime.terminations.load(Ordering::SeqCst), 1);
}

#[then(regex = r"^the session ends with (\d+) files cleaned$")]
fn ends_with_cleaned(w: &mut ControlWorld, cleaned: usize) {
    w.rig().wait_until("the session ended", |r| {
        r.events()
            .last()
            .is_some_and(|e| e.state == SessionState::Ended)
    });
    assert_eq!(w.rig().events().last().map(|e| e.cleaned), Some(cleaned));
}

#[then("the session ends without waiting 120 seconds and the deployed files are cleaned")]
fn ends_at_once(w: &mut ControlWorld) {
    // The fake game never reports anything on its own: only the stop ends it.
    w.rig().wait_until("the session ended", |r| {
        r.events()
            .last()
            .is_some_and(|e| e.state == SessionState::Ended)
    });
    assert!(disk_names(&w.rig().disk).is_empty());
    assert!(w
        .rig()
        .events()
        .iter()
        .all(|e| e.state != SessionState::NeverStarted));
}

#[then(regex = r#"^the error is "(no active session|stop failed)"$"#)]
fn error_is(w: &mut ControlWorld, wanted: String) {
    match (wanted.as_str(), w.error.as_ref()) {
        ("no active session", Some(error)) => assert_eq!(*error, PlayError::NoActiveSession),
        ("stop failed", Some(PlayError::StopFailed { detail })) => {
            assert!(detail.contains("access denied"), "{detail}")
        }
        (_, other) => panic!("unexpected result: {other:?}"),
    }
}

#[then("no file is cleaned")]
fn nothing_cleaned(w: &mut ControlWorld) {
    assert!(disk_has(&w.rig().disk, DEPLOYED_A));
    assert!(w
        .rig()
        .events()
        .iter()
        .all(|e| e.state == SessionState::Started));
}

pub async fn run() -> bool {
    let writer = ControlWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("session-control.feature"))
        .await;
    !writer.execution_has_failed()
}
