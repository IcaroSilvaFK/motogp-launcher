use crate::{
    AppError, ProcessInspector, SessionClock, SessionError, TrackOutcome, UrlOpener, POLL,
};
use motogp_domain::{launch_url, watch_step, GameInfo, WatchEvent, WatchState};
use std::path::Path;

/// Launches the game and tracks its session through ports.
pub struct SessionService<O, P, C> {
    opener: O,
    inspector: P,
    clock: C,
}

impl<O: UrlOpener, P: ProcessInspector, C: SessionClock> SessionService<O, P, C> {
    pub fn new(opener: O, inspector: P, clock: C) -> Self {
        Self {
            opener,
            inspector,
            clock,
        }
    }

    /// Opens the game's `steam://rungameid/<id>` URL.
    pub fn launch(&self, game: &GameInfo) -> Result<(), SessionError> {
        let url = launch_url(game)?;
        self.opener.open(&url).map_err(SessionError::Opener)
    }

    /// Polls every `POLL` on a fixed schedule (inspection time never drifts it)
    /// until a terminal event; each event goes to `emit`. `cancel` is checked
    /// before every inspection (so also after every wait); when it fires the
    /// tracking stops with `TrackOutcome::Cancelled` and no game event.
    pub fn track(
        &self,
        root: &Path,
        cancel: impl Fn() -> bool,
        mut emit: impl FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError> {
        let start = self.clock.now();
        let mut deadline = start;
        let mut state = WatchState::default();
        loop {
            if cancel() {
                return Ok(TrackOutcome::Cancelled);
            }
            let running = self.inspector.game_running(root)?;
            let elapsed = self.clock.now().saturating_sub(start);
            let (next, event) = watch_step(state, running, elapsed);
            state = next;
            if let Some(event) = event {
                emit(event);
            }
            if state.is_finished() {
                return Ok(TrackOutcome::Finished);
            }
            deadline += POLL;
            let remaining = deadline.saturating_sub(self.clock.now());
            if !remaining.is_zero() {
                self.clock.sleep(remaining);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;
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

    /// Reports `script[i]` on the i-th query (absent once the script ends)
    /// and records the clock time of every query.
    struct FakeInspector {
        clock: FakeClock,
        script: Vec<bool>,
        queries: RefCell<Vec<Duration>>,
        fail: bool,
        cost: Duration,
    }
    impl FakeInspector {
        fn new(clock: &FakeClock, script: Vec<bool>) -> Self {
            Self {
                clock: clock.clone(),
                script,
                queries: RefCell::default(),
                fail: false,
                cost: Duration::ZERO,
            }
        }
    }
    impl ProcessInspector for &FakeInspector {
        fn game_running(&self, _root: &Path) -> Result<bool, AppError> {
            if self.fail {
                return Err(AppError::Read {
                    location: "processes".into(),
                    detail: "denied".into(),
                });
            }
            let mut q = self.queries.borrow_mut();
            let i = q.len();
            q.push(self.clock.now());
            self.clock.0.set(self.clock.0.get() + self.cost);
            Ok(self.script.get(i).copied().unwrap_or(false))
        }
    }

    #[derive(Default)]
    struct FakeOpener {
        opened: RefCell<Vec<String>>,
        fail: bool,
    }
    impl UrlOpener for &FakeOpener {
        fn open(&self, url: &str) -> Result<(), AppError> {
            if self.fail {
                return Err(AppError::Read {
                    location: url.into(),
                    detail: "no handler".into(),
                });
            }
            self.opened.borrow_mut().push(url.into());
            Ok(())
        }
    }

    fn game(app_id: Option<u32>) -> GameInfo {
        GameInfo::new(PathBuf::from("lib").join("MotoGP25"), "MotoGP25", app_id)
    }

    fn track(script: Vec<bool>) -> (Vec<WatchEvent>, Vec<Duration>) {
        let (events, queries, outcome) = track_with(script, Duration::ZERO, || false);
        assert_eq!(outcome, TrackOutcome::Finished);
        (events, queries)
    }

    fn track_with(
        script: Vec<bool>,
        cost: Duration,
        cancel: impl Fn() -> bool,
    ) -> (Vec<WatchEvent>, Vec<Duration>, TrackOutcome) {
        let clock = FakeClock::default();
        let mut inspector = FakeInspector::new(&clock, script);
        inspector.cost = cost;
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        let mut events = Vec::new();
        let outcome = service
            .track(Path::new("root"), cancel, |e| events.push(e))
            .expect("track");
        let queries = inspector.queries.borrow().clone();
        (events, queries, outcome)
    }

    #[test]
    fn watcher_polls_every_two_seconds() {
        let (events, queries) = track(vec![true, true, false, false]);
        assert_eq!(events, vec![WatchEvent::Started, WatchEvent::Ended]);
        let secs: Vec<u64> = queries.iter().map(|d| d.as_secs()).collect();
        assert_eq!(secs, vec![0, 2, 4, 6]);
    }

    #[test]
    fn watcher_reports_never_started_at_120_seconds_without_sleeping_for_real() {
        let (events, queries) = track(vec![]);
        assert_eq!(events, vec![WatchEvent::NeverStarted]);
        assert_eq!(queries.last().map(|d| d.as_secs()), Some(120));
        assert_eq!(queries.len(), 61);
    }

    const INSPECTION: Duration = Duration::from_millis(250);

    #[test]
    fn inspection_time_does_not_drift_the_poll_schedule() {
        let (_, queries, _) = track_with(vec![true; 5], INSPECTION, || false);
        let millis: Vec<u128> = queries.iter().map(Duration::as_millis).collect();
        assert_eq!(&millis[..4], &[0, 2000, 4000, 6000]);
    }

    #[test]
    fn never_started_still_triggers_at_120_seconds_with_slow_inspections() {
        let (events, queries, _) = track_with(vec![], INSPECTION, || false);
        assert_eq!(events, vec![WatchEvent::NeverStarted]);
        assert_eq!(queries.last().map(Duration::as_secs), Some(120));
    }

    #[test]
    fn overrunning_poll_runs_the_next_one_immediately_without_drift() {
        let slow = Duration::from_secs(3);
        let (_, queries, _) = track_with(vec![true; 4], slow, || false);
        let secs: Vec<u64> = queries.iter().map(Duration::as_secs).collect();
        assert_eq!(&secs[..3], &[0, 3, 6]);
    }

    #[test]
    fn cancelled_before_start_never_inspects_nor_emits() {
        let (events, queries, outcome) = track_with(vec![true], Duration::ZERO, || true);
        assert_eq!(outcome, TrackOutcome::Cancelled);
        assert!(events.is_empty());
        assert!(queries.is_empty());
    }

    #[test]
    fn cancelled_while_running_stops_without_a_fake_ended() {
        let checks = Cell::new(0);
        let cancel = || {
            checks.set(checks.get() + 1);
            checks.get() > 3
        };
        let (events, queries, outcome) = track_with(vec![true; 10], Duration::ZERO, cancel);
        assert_eq!(outcome, TrackOutcome::Cancelled);
        assert_eq!(events, vec![WatchEvent::Started]);
        assert_eq!(queries.len(), 3);
    }

    #[test]
    fn watcher_propagates_inspector_errors() {
        let clock = FakeClock::default();
        let mut inspector = FakeInspector::new(&clock, vec![]);
        inspector.fail = true;
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        let error = service
            .track(Path::new("root"), || false, |_| {})
            .unwrap_err();
        assert_eq!(error.location(), "processes");
    }

    #[test]
    fn launch_opens_the_steam_url() {
        let clock = FakeClock::default();
        let inspector = FakeInspector::new(&clock, vec![]);
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        service.launch(&game(Some(123))).expect("launch");
        assert_eq!(*opener.opened.borrow(), vec!["steam://rungameid/123"]);
    }

    #[test]
    fn launch_without_app_id_does_not_open_anything() {
        let clock = FakeClock::default();
        let inspector = FakeInspector::new(&clock, vec![]);
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        let error = service.launch(&game(None)).unwrap_err();
        assert_eq!(
            error.to_string(),
            "game App ID not found; open the game from Steam once"
        );
        assert!(opener.opened.borrow().is_empty());
    }

    #[test]
    fn launch_reports_opener_failures() {
        let clock = FakeClock::default();
        let inspector = FakeInspector::new(&clock, vec![]);
        let opener = FakeOpener {
            fail: true,
            ..Default::default()
        };
        let service = SessionService::new(&opener, &inspector, clock);
        let error = service.launch(&game(Some(1))).unwrap_err();
        assert!(matches!(error, SessionError::Opener(_)));
        assert!(error.to_string().contains("no handler"));
    }
}
