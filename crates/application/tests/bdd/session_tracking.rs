//! Steps for `tests/features/session-tracking.feature` (issue #7), fake clock and processes only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, ProcessInspector, SessionClock, SessionService, UrlOpener};
use motogp_domain::WatchEvent;
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Default)]
struct FakeClock(Rc<Cell<Duration>>);

impl SessionClock for FakeClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
    fn sleep(&self, d: Duration) {
        self.0.set(self.0.get() + d);
    }
}

#[derive(Clone, Default)]
struct FakeInspector {
    clock: FakeClock,
    queries: Rc<RefCell<Vec<Duration>>>,
}

impl ProcessInspector for FakeInspector {
    fn game_running(&self, _root: &Path) -> Result<bool, AppError> {
        let mut queries = self.queries.borrow_mut();
        queries.push(self.clock.now());
        // Present for the first two polls, then gone.
        Ok(queries.len() <= 2)
    }
}

struct NoOpener;

impl UrlOpener for NoOpener {
    fn open(&self, _url: &str) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default, World)]
struct TrackingWorld {
    inspector: FakeInspector,
    events: Vec<WatchEvent>,
}

impl std::fmt::Debug for TrackingWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TrackingWorld")
    }
}

#[given("a controlled clock and process inspector")]
fn controlled(w: &mut TrackingWorld) {
    w.inspector = FakeInspector::default();
}

#[when("I track a session")]
fn track(w: &mut TrackingWorld) {
    let clock = w.inspector.clock.clone();
    let service = SessionService::new(NoOpener, w.inspector.clone(), clock);
    let mut events = Vec::new();
    service
        .track(Path::new("root"), || false, |e| events.push(e))
        .expect("track");
    w.events = events;
}

#[then("the queries are scheduled at 2-second intervals")]
fn two_second_intervals(w: &mut TrackingWorld) {
    let secs: Vec<u64> = w
        .inspector
        .queries
        .borrow()
        .iter()
        .map(Duration::as_secs)
        .collect();
    assert_eq!(secs, vec![0, 2, 4, 6]);
    assert_eq!(w.events, vec![WatchEvent::Started, WatchEvent::Ended]);
}

/// Runs only `session-tracking.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = TrackingWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("session-tracking.feature"))
        .await;
    !writer.execution_has_failed()
}
