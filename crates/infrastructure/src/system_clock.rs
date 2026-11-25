use motogp_application::SessionClock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Real monotonic clock; `now` is the time since construction.
#[derive(Debug, Clone, Copy)]
pub struct MonotonicClock {
    origin: Instant,
}

impl MonotonicClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionClock for MonotonicClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn unix_now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_advances_after_sleeping() {
        let clock = MonotonicClock::new();
        let before = clock.now();
        clock.sleep(Duration::from_millis(5));
        assert!(clock.now() >= before + Duration::from_millis(5));
    }

    #[test]
    fn wall_clock_is_a_recent_unix_time_in_milliseconds() {
        // 2024-01-01 in Unix milliseconds: any real clock is past it.
        assert!(MonotonicClock::new().unix_now_ms() > 1_704_067_200_000);
    }
}
