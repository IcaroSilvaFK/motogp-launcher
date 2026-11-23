use motogp_application::{AppError, ScannedEntry, StorageScanner};
use std::fs;
use std::path::Path;

/// `StorageScanner` over the real file system. Links and junctions are never
/// followed, so a loop cannot hang the scan and nothing outside the data
/// folder (e.g. the game folder) is counted.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsStorageScanner;

impl StorageScanner for FsStorageScanner {
    fn scan(&self, root: &Path) -> Result<Vec<ScannedEntry>, AppError> {
        let children = match fs::read_dir(root) {
            Ok(children) => children,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(AppError::Read {
                    location: root.display().to_string(),
                    detail: error.to_string(),
                })
            }
        };
        // An entry that cannot be read (locked, denied, removed meanwhile) is skipped.
        Ok(children
            .filter_map(Result::ok)
            .filter_map(|child| {
                let meta = fs::symlink_metadata(child.path()).ok()?;
                let is_dir = meta.is_dir();
                Some(ScannedEntry {
                    name: child.file_name().to_string_lossy().into_owned(),
                    is_dir,
                    bytes: if is_link(&meta) {
                        0
                    } else if is_dir {
                        dir_size(&child.path())
                    } else {
                        meta.len()
                    },
                })
            })
            .collect())
    }

    fn free_space(&self, root: &Path) -> Result<u64, AppError> {
        // A fresh install has no folder yet: measure the nearest existing parent.
        let existing = root
            .ancestors()
            .find(|p| p.exists())
            .unwrap_or_else(|| Path::new("."));
        fs2::available_space(existing).map_err(|error| AppError::Read {
            location: root.display().to_string(),
            detail: error.to_string(),
        })
    }
}

/// Symlinks and junctions are never followed.
fn is_link(meta: &fs::Metadata) -> bool {
    meta.file_type().is_symlink() || is_reparse_point(meta)
}

/// Recursive size of a directory without following links; unreadable parts
/// count as 0.
fn dir_size(dir: &Path) -> u64 {
    let Ok(children) = fs::read_dir(dir) else {
        return 0;
    };
    children
        .filter_map(Result::ok)
        .map(|child| {
            let Ok(meta) = fs::symlink_metadata(child.path()) else {
                return 0;
            };

            if is_link(&meta) {
                0
            } else if meta.is_dir() {
                dir_size(&child.path())
            } else {
                meta.len()
            }
        })
        .fold(0u64, u64::saturating_add)
}

/// Windows junctions are reparse points that `is_symlink` does not report.
#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![0u8; bytes]).unwrap();
    }

    #[test]
    fn measures_children_recursively() {
        let tmp = TempDir::new().unwrap();
        write(&tmp.path().join("library/a/x.pak"), 30);
        write(&tmp.path().join("library/y.pak"), 20);
        write(&tmp.path().join("config.json"), 7);
        let mut entries = FsStorageScanner.scan(tmp.path()).unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(
            entries,
            vec![
                ScannedEntry {
                    name: "config.json".into(),
                    is_dir: false,
                    bytes: 7
                },
                ScannedEntry {
                    name: "library".into(),
                    is_dir: true,
                    bytes: 50
                },
            ]
        );
    }

    #[test]
    fn a_missing_root_is_empty_and_still_reports_free_space() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("not-yet");
        assert_eq!(FsStorageScanner.scan(&missing).unwrap(), vec![]);
        assert!(FsStorageScanner.free_space(&missing).unwrap() > 0);
    }

    #[test]
    fn a_root_that_is_a_file_is_a_read_error() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("data");
        write(&file, 1);
        let error = FsStorageScanner.scan(&file).unwrap_err();
        assert!(matches!(error, AppError::Read { .. }));
        assert_eq!(error.location(), file.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("outside");
        write(&target.join("big.bin"), 1000);
        let root = tmp.path().join("data");
        fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&target, root.join("link")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        let entries = FsStorageScanner.scan(&root).unwrap();
        assert!(entries.iter().all(|e| e.bytes == 0), "{entries:?}");
    }
}
