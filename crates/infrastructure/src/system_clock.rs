use motogp_application::SessionClock;
use std::time::{Duration, Instant};

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
}
