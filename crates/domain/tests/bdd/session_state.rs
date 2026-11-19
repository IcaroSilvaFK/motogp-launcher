//! Steps for `tests/features/session-state.feature` (issue #7). Pure rules only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{launch_url, watch_step, GameInfo, WatchEvent, WatchState};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Default, World)]
struct SessionWorld {
    app_id: Option<u32>,
    launch: Option<Result<String, String>>,
    state: WatchState,
    events: Vec<WatchEvent>,
    last_event: Option<WatchEvent>,
}

impl SessionWorld {
    fn poll(&mut self, running: bool, secs: u64) {
        let (state, event) = watch_step(self.state, running, Duration::from_secs(secs));
        self.state = state;
        self.last_event = event;
        self.events.extend(event);
    }

    fn count(&self, wanted: WatchEvent) -> usize {
        self.events.iter().filter(|e| **e == wanted).count()
    }
}

#[given(expr = "a game with App ID {int}")]
fn with_app_id(w: &mut SessionWorld, id: u32) {
    w.app_id = Some(id);
}

#[given("a game without an App ID")]
fn without_app_id(w: &mut SessionWorld) {
    w.app_id = None;
}

#[when("I prepare its launch")]
fn prepare(w: &mut SessionWorld) {
    let game = GameInfo::new(PathBuf::from("MotoGP25"), "MotoGP25", w.app_id);
    w.launch = Some(launch_url(&game).map_err(|e| e.to_string()));
}

#[then(expr = "I get {string}")]
fn i_get(w: &mut SessionWorld, expected: String) {
    let got = match w.launch.as_ref().expect("no launch prepared") {
        Ok(url) => url,
        Err(message) => message,
    };
    assert_eq!(got, &expected);
}

#[given("the first poll finds the game and emits Started")]
fn first_poll_started(w: &mut SessionWorld) {
    w.poll(true, 0);
    assert_eq!(w.last_event, Some(WatchEvent::Started));
}

#[when("the next poll does not find the process")]
fn next_poll_absent(w: &mut SessionWorld) {
    w.poll(false, 2);
}

#[when("the following poll also does not find the process")]
fn following_poll_absent(w: &mut SessionWorld) {
    w.poll(false, 4);
}

#[then("no end is emitted")]
fn no_end(w: &mut SessionWorld) {
    assert_eq!(w.count(WatchEvent::Ended), 0);
    assert_eq!(w.count(WatchEvent::NeverStarted), 0);
}

#[then("Ended is emitted exactly once")]
fn ended_once(w: &mut SessionWorld) {
    assert_eq!(w.count(WatchEvent::Ended), 1);
    w.poll(false, 6);
    assert_eq!(w.count(WatchEvent::Ended), 1);
}

#[given("the poll sequence present, absent, present, absent")]
fn flapping_sequence(_w: &mut SessionWorld) {}

#[when("I advance the state machine")]
fn advance(w: &mut SessionWorld) {
    for (i, running) in [true, false, true, false].into_iter().enumerate() {
        w.poll(running, 2 * i as u64);
    }
}

#[then("Started is emitted on the first presence")]
fn started_on_first(w: &mut SessionWorld) {
    assert_eq!(w.events.first(), Some(&WatchEvent::Started));
    assert_eq!(w.count(WatchEvent::Started), 1);
}

#[then("Ended is not emitted")]
fn ended_not_emitted(w: &mut SessionWorld) {
    assert_eq!(w.count(WatchEvent::Ended), 0);
}

#[given("no poll has found the process")]
fn never_found(w: &mut SessionWorld) {
    w.poll(false, 0);
}

#[when(expr = "the elapsed time reaches {int} seconds")]
fn elapsed_reaches(w: &mut SessionWorld, secs: u64) {
    w.poll(false, secs);
}

#[when(expr = "the time reaches {int} seconds")]
fn time_reaches(w: &mut SessionWorld, secs: u64) {
    w.poll(false, secs);
}

#[then("NeverStarted is emitted exactly once")]
fn never_started_once(w: &mut SessionWorld) {
    assert_eq!(w.count(WatchEvent::NeverStarted), 1);
    w.poll(false, 240);
    assert_eq!(w.count(WatchEvent::NeverStarted), 1);
}

/// Runs only `session-state.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = SessionWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("session-state.feature"))
        .await;
    !writer.execution_has_failed()
}
