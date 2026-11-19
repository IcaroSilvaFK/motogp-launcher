//! Single-instance lock: an exclusive OS file lock held for the app lifetime.
use fs2::FileExt;
use motogp_application::{AppError, InstanceGuard, InstanceLock};
use std::fs::OpenOptions;
use std::path::PathBuf;

/// `InstanceLock` backed by an exclusive lock on a file in the app data
/// directory. The OS releases it when the process ends, even after a crash.
#[derive(Debug, Clone)]
pub struct FileInstanceLock {
    path: PathBuf,
}

impl FileInstanceLock {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl InstanceLock for FileInstanceLock {
    fn acquire(&self) -> Result<Option<InstanceGuard>, AppError> {
        let location = self.path.display().to_string();
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.path)
            .map_err(|error| AppError::Write {
                location: location.clone(),
                detail: error.to_string(),
            })?;
        match file.try_lock_exclusive() {
            // The lock lives as long as the open handle.
            Ok(()) => Ok(Some(Box::new(file))),
            Err(error) if error.kind() == fs2::lock_contended_error().kind() => Ok(None),
            Err(error) => Err(AppError::Write {
                location,
                detail: error.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::InstanceLock;

    #[test]
    fn a_second_acquisition_fails_while_the_first_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("instance.lock");
        let first = FileInstanceLock::new(path.clone());
        let second = FileInstanceLock::new(path);

        let held = first.acquire().unwrap().expect("first instance gets it");

        assert!(second.acquire().unwrap().is_none(), "second is refused");
        drop(held);
        assert!(
            second.acquire().unwrap().is_some(),
            "free again after the first exits"
        );
    }

    #[test]
    fn an_unwritable_location_is_an_error_not_a_silent_success() {
        let dir = tempfile::tempdir().unwrap();
        let lock = FileInstanceLock::new(dir.path().join("missing").join("instance.lock"));

        assert!(lock.acquire().is_err());
    }
}
