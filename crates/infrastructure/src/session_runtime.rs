//! Adapters for the session lock and background execution (plain std).
use motogp_application::{BackgroundRunner, SessionLock};
use std::sync::atomic::{AtomicBool, Ordering};

/// `SessionLock` backed by an atomic flag.
#[derive(Debug, Default)]
pub struct AtomicSessionLock(AtomicBool);

impl SessionLock for AtomicSessionLock {
    fn try_acquire(&self) -> bool {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn release(&self) {
        self.0.store(false, Ordering::Release);
    }
}

/// `BackgroundRunner` that runs each task on its own named OS thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadRunner;

impl BackgroundRunner for ThreadRunner {
    fn spawn(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        // A failure to create the thread is a resource exhaustion we cannot
        // recover from here; the task (and its lock) would never run.
        std::thread::Builder::new()
            .name("motogp-session".into())
            .spawn(task)
            .expect("could not start the session thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::{BackgroundRunner, SessionLock};
    use std::sync::mpsc;
    use std::sync::Arc;

    #[test]
    fn lock_is_exclusive_until_released() {
        let lock = AtomicSessionLock::default();
        assert!(lock.try_acquire());
        assert!(!lock.try_acquire());
        lock.release();
        assert!(lock.try_acquire());
    }

    #[test]
    fn only_one_thread_wins_a_race_for_the_lock() {
        let lock = Arc::new(AtomicSessionLock::default());
        let winners: usize = (0..8)
            .map(|_| {
                let lock = lock.clone();
                std::thread::spawn(move || lock.try_acquire())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| usize::from(handle.join().unwrap()))
            .sum();
        assert_eq!(winners, 1);
    }

    #[test]
    fn runner_executes_the_task_on_another_thread() {
        let (sender, receiver) = mpsc::channel();
        let caller = std::thread::current().id();
        ThreadRunner.spawn(Box::new(move || {
            sender.send(std::thread::current().id()).unwrap();
        }));
        let worker = receiver
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("the task ran");
        assert_ne!(worker, caller);
    }
}
