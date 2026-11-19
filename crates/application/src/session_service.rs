use crate::{AppError, ProcessInspector, SessionClock, SessionError, UrlOpener, POLL};
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

    /// Polls every `POLL` until a terminal event; each event goes to `emit`.
    pub fn track(&self, root: &Path, mut emit: impl FnMut(WatchEvent)) -> Result<(), AppError> {
        let start = self.clock.now();
        let mut state = WatchState::default();
        loop {
            let running = self.inspector.game_running(root)?;
            let elapsed = self.clock.now().saturating_sub(start);
            let (next, event) = watch_step(state, running, elapsed);
            state = next;
            if let Some(event) = event {
                emit(event);
            }
            if state.is_finished() {
                return Ok(());
            }
            self.clock.sleep(POLL);
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
    }
    impl FakeInspector {
        fn new(clock: &FakeClock, script: Vec<bool>) -> Self {
            Self {
                clock: clock.clone(),
                script,
                queries: RefCell::default(),
                fail: false,
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
        let clock = FakeClock::default();
        let inspector = FakeInspector::new(&clock, script);
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        let mut events = Vec::new();
        service
            .track(Path::new("root"), |e| events.push(e))
            .expect("track");
        let queries = inspector.queries.borrow().clone();
        (events, queries)
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

    #[test]
    fn watcher_propagates_inspector_errors() {
        let clock = FakeClock::default();
        let mut inspector = FakeInspector::new(&clock, vec![]);
        inspector.fail = true;
        let opener = FakeOpener::default();
        let service = SessionService::new(&opener, &inspector, clock);
        let error = service.track(Path::new("root"), |_| {}).unwrap_err();
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
