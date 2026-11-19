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
