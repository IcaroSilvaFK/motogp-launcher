//! Pure rules to launch the game and to follow its session (no fs, process or clock).
use crate::GameInfo;
use std::fmt;
use std::time::Duration;

/// Without any observation up to this elapsed time (inclusive) the game never started.
pub const NEVER_STARTED_AFTER: Duration = Duration::from_secs(120);

/// Consecutive absences, after the first observation, that end the session.
pub const MISSING_POLLS_TO_END: u8 = 2;

/// Why a launch could not be prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchError {
    /// The installation has no Steam App ID.
    MissingAppId,
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::MissingAppId => {
                write!(f, "game App ID not found; open the game from Steam once")
            }
        }
    }
}

impl std::error::Error for LaunchError {}

/// `steam://rungameid/<id>` for a game with a detected App ID.
pub fn launch_url(game: &GameInfo) -> Result<String, LaunchError> {
    game.app_id
        .map(|id| format!("steam://rungameid/{id}"))
        .ok_or(LaunchError::MissingAppId)
}

/// What a session observation can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEvent {
    Started,
    Ended,
    NeverStarted,
}

/// State of the session watcher between two polls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WatchState {
    started: bool,
    missing: u8,
    finished: bool,
}

impl WatchState {
    /// True after `Ended` or `NeverStarted`: nothing else is ever emitted.
    pub fn is_finished(&self) -> bool {
        self.finished
    }
}

/// Advances the state machine with one observation.
/// `running` says whether a game process was seen; `elapsed` is the time
/// since tracking began.
pub fn watch_step(
    state: WatchState,
    running: bool,
    elapsed: Duration,
) -> (WatchState, Option<WatchEvent>) {
    if state.finished {
        return (state, None);
    }
    let mut next = state;
    if running {
        next.missing = 0;
        if !next.started {
            next.started = true;
            return (next, Some(WatchEvent::Started));
        }
        return (next, None);
    }
    if next.started {
        next.missing += 1;
        if next.missing >= MISSING_POLLS_TO_END {
            next.finished = true;
            return (next, Some(WatchEvent::Ended));
        }
    } else if elapsed >= NEVER_STARTED_AFTER {
        next.finished = true;
        return (next, Some(WatchEvent::NeverStarted));
    }
    (next, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn game(app_id: Option<u32>) -> GameInfo {
        GameInfo::new(PathBuf::from("lib").join("MotoGP25"), "MotoGP25", app_id)
    }

    const T0: Duration = Duration::ZERO;

    /// Feeds `(running, elapsed_secs)` observations and collects the events.
    fn run(polls: &[(bool, u64)]) -> (WatchState, Vec<Option<WatchEvent>>) {
        let mut state = WatchState::default();
        let mut events = Vec::new();
        for &(running, secs) in polls {
            let (next, event) = watch_step(state, running, Duration::from_secs(secs));
            state = next;
            events.push(event);
        }
        (state, events)
    }

    #[test]
    fn launch_url_uses_app_id() {
        assert_eq!(
            launch_url(&game(Some(123))).as_deref(),
            Ok("steam://rungameid/123")
        );
    }

    #[test]
    fn launch_url_without_app_id_errors() {
        let error = launch_url(&game(None)).unwrap_err();
        assert_eq!(error, LaunchError::MissingAppId);
        assert_eq!(
            error.to_string(),
            "game App ID not found; open the game from Steam once"
        );
    }

    #[test]
    fn started_then_ended_after_two_missing_polls() {
        let (state, events) = run(&[(true, 0), (false, 2), (false, 4)]);
        assert_eq!(
            events,
            vec![Some(WatchEvent::Started), None, Some(WatchEvent::Ended)]
        );
        assert!(state.is_finished());
    }

    #[test]
    fn flapping_resets_missing_counter() {
        let (state, events) = run(&[(true, 0), (false, 2), (true, 4), (false, 6)]);
        assert_eq!(events, vec![Some(WatchEvent::Started), None, None, None]);
        assert!(!state.is_finished());
    }

    #[test]
    fn never_started_after_120s() {
        let (state, events) = run(&[(false, 0), (false, 119)]);
        assert_eq!(events, vec![None, None]);
        assert!(!state.is_finished());
        let (state, event) = watch_step(state, false, Duration::from_secs(120));
        assert_eq!(event, Some(WatchEvent::NeverStarted));
        assert!(state.is_finished());
    }

    #[test]
    fn terminal_watch_event_is_not_repeated() {
        let (state, events) = run(&[(true, 0), (false, 2), (false, 4), (false, 6), (true, 8)]);
        assert_eq!(events[2], Some(WatchEvent::Ended));
        assert_eq!(events[3..], [None, None]);
        assert!(state.is_finished());

        let (state, _) = run(&[(false, 120)]);
        let (_, again) = watch_step(state, false, Duration::from_secs(240));
        assert_eq!(again, None);
        let (_, started) = watch_step(state, true, T0);
        assert_eq!(started, None);
    }

    #[test]
    fn presence_after_the_deadline_still_starts() {
        let (_, events) = run(&[(true, 130)]);
        assert_eq!(events, vec![Some(WatchEvent::Started)]);
    }
}
