//! `DownloadStaging` adapter: `<data dir>/downloads/<entry id>/` on disk.
use motogp_application::{DownloadError, DownloadStaging};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Staging directories of catalog downloads under one root.
#[derive(Debug, Clone)]
pub struct FsDownloadStaging {
    root: PathBuf,
}

impl FsDownloadStaging {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The entry directory, only for a plain single-component id (no path tricks).
    fn entry_dir(&self, entry_id: &str) -> Option<PathBuf> {
        let mut components = Path::new(entry_id).components();
        match (components.next(), components.next()) {
            (Some(Component::Normal(name)), None)
                if name == entry_id && !entry_id.contains(['/', '\\']) =>
            {
                Some(self.root.join(name))
            }
            _ => None,
        }
    }
}

fn storage(error: std::io::Error) -> DownloadError {
    DownloadError::Storage(error.to_string())
}

impl DownloadStaging for FsDownloadStaging {
    fn create(&self, entry_id: &str) -> Result<PathBuf, DownloadError> {
        let dir = self
            .entry_dir(entry_id)
            .ok_or_else(|| DownloadError::Storage(format!("invalid entry id \"{entry_id}\"")))?;
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(storage)?;
        }
        fs::create_dir_all(&dir).map_err(storage)?;
        Ok(dir)
    }

    fn file_size(&self, file: &Path) -> Result<u64, DownloadError> {
        fs::metadata(file).map(|m| m.len()).map_err(storage)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), DownloadError> {
        fs::rename(from, to).map_err(storage)
    }

    fn remove(&self, entry_id: &str) {
        if let Some(dir) = self.entry_dir(entry_id) {
            // Idempotent and best effort: a missing directory is already clean.
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_gives_a_fresh_empty_directory() {
        let tmp = tempdir().unwrap();
        let staging = FsDownloadStaging::new(tmp.path().join("downloads"));
        let dir = staging.create("rebalanced-ai").unwrap();
        assert_eq!(dir, tmp.path().join("downloads").join("rebalanced-ai"));
        fs::write(dir.join("old.part"), b"x").unwrap();
        let again = staging.create("rebalanced-ai").unwrap();
        assert_eq!(fs::read_dir(again).unwrap().count(), 0);
    }

    #[test]
    fn remove_is_idempotent() {
        let tmp = tempdir().unwrap();
        let staging = FsDownloadStaging::new(tmp.path());
        let dir = staging.create("a").unwrap();
        staging.remove("a");
        assert!(!dir.exists());
        staging.remove("a");
        staging.remove("never-created");
    }

    #[test]
    fn size_and_rename_work_on_staged_files() {
        let tmp = tempdir().unwrap();
        let staging = FsDownloadStaging::new(tmp.path());
        let dir = staging.create("a").unwrap();
        let part = dir.join("m.zip.part");
        fs::write(&part, b"12345").unwrap();
        assert_eq!(staging.file_size(&part).unwrap(), 5);
        let done = dir.join("m.zip");
        staging.rename(&part, &done).unwrap();
        assert_eq!(fs::read(&done).unwrap(), b"12345");
        assert!(matches!(
            staging.file_size(&part),
            Err(DownloadError::Storage(_))
        ));
        assert!(matches!(
            staging.rename(&part, &done),
            Err(DownloadError::Storage(_))
        ));
    }

    #[test]
    fn ids_that_escape_the_root_are_refused() {
        let tmp = tempdir().unwrap();
        let staging = FsDownloadStaging::new(tmp.path().join("downloads"));
        for id in ["", "..", "a/b", "../x", "a\\b", "."] {
            assert!(
                matches!(staging.create(id), Err(DownloadError::Storage(_))),
                "{id:?}"
            );
            staging.remove(id);
        }
    }
}
