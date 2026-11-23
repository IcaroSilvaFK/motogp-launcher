use crate::{AppError, FolderOpener, StorageScanner};
use motogp_domain::{classify_entry, StorageUsage};
use std::path::PathBuf;

/// Use case: how much space the launcher data folder uses.
pub struct StorageService<S, O> {
    root: PathBuf,
    scanner: S,
    opener: O,
}

impl<S: StorageScanner, O: FolderOpener> StorageService<S, O> {
    pub fn new(root: PathBuf, scanner: S, opener: O) -> Self {
        Self {
            root,
            scanner,
            opener,
        }
    }

    /// Measures the data folder. Unknown free space is `None`, never an error;
    /// a folder that cannot be scanned is.
    pub fn usage(&self) -> Result<StorageUsage, AppError> {
        let entries = self.scanner.scan(&self.root)?;
        let free = self.scanner.free_space(&self.root).ok();
        Ok(StorageUsage::from_entries(
            self.root.clone(),
            entries
                .iter()
                .map(|e| (classify_entry(&e.name, e.is_dir), e.bytes)),
            free,
        ))
    }

    /// Opens exactly the data folder; the caller never chooses the path.
    pub fn open_data_folder(&self) -> Result<(), AppError> {
        self.opener.open_folder(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScannedEntry;
    use motogp_domain::StorageCategory;
    use std::cell::RefCell;
    use std::path::Path;

    struct FakeScanner {
        entries: Result<Vec<ScannedEntry>, AppError>,
        free: Result<u64, AppError>,
    }

    impl StorageScanner for FakeScanner {
        fn scan(&self, _root: &Path) -> Result<Vec<ScannedEntry>, AppError> {
            self.entries.clone()
        }
        fn free_space(&self, _root: &Path) -> Result<u64, AppError> {
            self.free.clone()
        }
    }

    #[derive(Default)]
    struct FakeOpener {
        opened: RefCell<Vec<PathBuf>>,
        fail: bool,
    }

    impl FolderOpener for &FakeOpener {
        fn open_folder(&self, path: &Path) -> Result<(), AppError> {
            self.opened.borrow_mut().push(path.to_path_buf());
            if self.fail {
                return Err(AppError::Read {
                    location: path.display().to_string(),
                    detail: "no file manager".into(),
                });
            }
            Ok(())
        }
    }

    fn entry(name: &str, is_dir: bool, bytes: u64) -> ScannedEntry {
        ScannedEntry {
            name: name.into(),
            is_dir,
            bytes,
        }
    }

    fn denied() -> AppError {
        AppError::Read {
            location: "x".into(),
            detail: "denied".into(),
        }
    }

    #[test]
    fn groups_and_sums_entries() {
        let scanner = FakeScanner {
            entries: Ok(vec![
                entry("library", true, 1000),
                entry("config.json", false, 10),
                entry("crash.dmp", false, 5),
            ]),
            free: Ok(77),
        };
        let opener = FakeOpener::default();
        // Non-ASCII on purpose: Windows user folders often contain accents.
        let root = PathBuf::from("C:/Users/Usuário Padrão/data");
        let usage = StorageService::new(root.clone(), scanner, &opener)
            .usage()
            .unwrap();
        assert_eq!(usage.root, root);
        assert_eq!(usage.bytes_of(StorageCategory::Mods), 1000);
        assert_eq!(usage.bytes_of(StorageCategory::LauncherData), 10);
        assert_eq!(usage.bytes_of(StorageCategory::Other), 5);
        assert_eq!(usage.free_bytes, Some(77));
    }

    #[test]
    fn unknown_free_space_is_none() {
        let scanner = FakeScanner {
            entries: Ok(vec![]),
            free: Err(denied()),
        };
        let opener = FakeOpener::default();
        let usage = StorageService::new(PathBuf::from("r"), scanner, &opener)
            .usage()
            .unwrap();
        assert_eq!(usage.free_bytes, None);
        assert_eq!(usage.total_bytes(), 0);
    }

    #[test]
    fn a_scan_failure_is_an_error() {
        let scanner = FakeScanner {
            entries: Err(denied()),
            free: Ok(1),
        };
        let opener = FakeOpener::default();
        let error = StorageService::new(PathBuf::from("r"), scanner, &opener)
            .usage()
            .unwrap_err();
        assert_eq!(error, denied());
    }

    #[test]
    fn opens_only_the_data_folder_and_reports_failures() {
        let scanner = || FakeScanner {
            entries: Ok(vec![]),
            free: Ok(1),
        };
        let ok = FakeOpener::default();
        StorageService::new(PathBuf::from("data"), scanner(), &ok)
            .open_data_folder()
            .unwrap();
        assert_eq!(*ok.opened.borrow(), vec![PathBuf::from("data")]);

        let failing = FakeOpener {
            fail: true,
            ..Default::default()
        };
        let error = StorageService::new(PathBuf::from("data"), scanner(), &failing)
            .open_data_folder()
            .unwrap_err();
        assert!(matches!(error, AppError::Read { .. }));
    }
}
