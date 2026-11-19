use motogp_application::Clock;
use std::time::{Duration, Instant};

/// Real monotonic clock; `now` is the time since construction.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        todo!()
    }

    fn sleep(&self, duration: Duration) {
        let _ = duration;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_advances_after_sleeping() {
        let clock = SystemClock::new();
        let before = clock.now();
        clock.sleep(Duration::from_millis(5));
        assert!(clock.now() >= before + Duration::from_millis(5));
    }
}
