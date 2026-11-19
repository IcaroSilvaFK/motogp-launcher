//! `ArchiveReader` adapter: ZIP extraction. Entries are flattened, unsafe
//! names (no `enclosed_name()`) are ignored and nothing is overwritten.
use motogp_application::{AppError, ArchiveLimit, ArchiveReader, LibraryError};
use motogp_domain::is_mod_file;
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

/// Characters that are invalid in Windows file names (or control characters).
fn has_invalid_chars(name: &str) -> bool {
    name.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/')
    })
}

/// Resource limits enforced while extracting (zip bomb protection). Declared
/// sizes are never trusted: bytes are counted while streaming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipLimits {
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_entries: usize,
}

impl Default for ZipLimits {
    fn default() -> Self {
        Self {
            max_entry_bytes: 8 * 1024 * 1024 * 1024,
            max_total_bytes: 16 * 1024 * 1024 * 1024,
            max_entries: 10_000,
        }
    }
}

/// Reads ZIP archives with the `zip` crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZipArchiveReader {
    limits: ZipLimits,
}

impl ZipArchiveReader {
    pub fn with_limits(limits: ZipLimits) -> Self {
        Self { limits }
    }
}

impl ArchiveReader for ZipArchiveReader {
    fn extract_flat(&self, archive: &Path, dest_dir: &Path) -> Result<Vec<String>, LibraryError> {
        let archive_location = archive.display().to_string();
        let corrupted = |e: &dyn std::fmt::Display| {
            LibraryError::Storage(AppError::Corrupted {
                location: archive_location.clone(),
                detail: e.to_string(),
            })
        };
        let file = fs::File::open(archive).map_err(|e| {
            LibraryError::Storage(AppError::Read {
                location: archive_location.clone(),
                detail: e.to_string(),
            })
        })?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| corrupted(&e))?;
        if zip.len() > self.limits.max_entries {
            return Err(LibraryError::ArchiveLimitExceeded {
                limit: ArchiveLimit::EntryCount,
            });
        }
        let mut total_bytes: u64 = 0;

        let mut written: Vec<String> = Vec::new();
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| corrupted(&e))?;
            if entry.is_dir() {
                continue;
            }
            // Entries without a safe (enclosed) name never reach the disk.
            let Some(safe_path) = entry.enclosed_name() else {
                continue;
            };
            let Some(name) = safe_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if !is_mod_file(&name) || has_invalid_chars(&name) {
                continue;
            }
            if written.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
                return Err(LibraryError::NameCollision { name });
            }
            let target = dest_dir.join(&name);
            let write_error = |e: io::Error| {
                LibraryError::Storage(AppError::Write {
                    location: target.display().to_string(),
                    detail: e.to_string(),
                })
            };
            // `create_new` never overwrites an existing file.
            let mut out = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(write_error)?;
            // Read at most one byte past the caps so an overflow is detected
            // without trusting the declared size.
            let allowed = self
                .limits
                .max_entry_bytes
                .min(self.limits.max_total_bytes - total_bytes);
            let copied = io::copy(&mut (&mut entry).take(allowed.saturating_add(1)), &mut out)
                .map_err(write_error)?;
            if copied > allowed {
                let limit = if copied > self.limits.max_entry_bytes {
                    ArchiveLimit::EntryBytes
                } else {
                    ArchiveLimit::TotalBytes
                };
                // The caller discards the whole staging directory.
                return Err(LibraryError::ArchiveLimitExceeded { limit });
            }
            total_bytes += copied;
            written.push(name);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::{AppError, ArchiveLimit};
    use std::fs;
    use std::io::Write;

    fn build_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, content) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap();
    }

    fn sorted(mut names: Vec<String>) -> Vec<String> {
        names.sort();
        names
    }

    #[test]
    fn flattens_nested_folders_and_keeps_only_mod_files() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mods.zip");
        build_zip(
            &zip,
            &[
                ("x/y/A_P.pak", b"a"),
                ("x/y/A_P.utoc", b"u"),
                ("B.PAK", b"b"),
                ("readme.txt", b"hi"),
                ("docs/", b""),
            ],
        );
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        let names = ZipArchiveReader::default()
            .extract_flat(&zip, &out)
            .unwrap();

        assert_eq!(sorted(names), vec!["A_P.pak", "A_P.utoc", "B.PAK"]);
        assert_eq!(fs::read(out.join("A_P.pak")).unwrap(), b"a");
        assert!(!out.join("readme.txt").exists());
        assert!(!out.join("x").exists());
    }

    #[test]
    fn entries_without_a_safe_name_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("evil.zip");
        build_zip(
            &zip,
            &[
                ("../evil.pak", b"evil"),
                ("/abs.pak", b"abs"),
                ("a/../../up.pak", b"up"),
                ("c:colon.pak", b"colon"),
                ("ok.pak", b"ok"),
            ],
        );
        let out = dir.path().join("lib").join("staging");
        fs::create_dir_all(&out).unwrap();

        let names = ZipArchiveReader::default()
            .extract_flat(&zip, &out)
            .unwrap();

        assert_eq!(names, vec!["ok.pak"]);
        assert!(!dir.path().join("evil.pak").exists());
        assert!(!dir.path().join("lib").join("evil.pak").exists());
        assert!(!dir.path().join("up.pak").exists());
    }

    #[test]
    fn colliding_flattened_names_are_rejected_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("collide.zip");
        build_zip(&zip, &[("x/A.pak", b"first"), ("y/a.PAK", b"second")]);
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        let error = ZipArchiveReader::default()
            .extract_flat(&zip, &out)
            .unwrap_err();

        assert!(
            matches!(error, LibraryError::NameCollision { .. }),
            "{error:?}"
        );
        assert_eq!(fs::read(out.join("A.pak")).unwrap(), b"first");
    }

    #[test]
    fn existing_files_in_the_destination_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mods.zip");
        build_zip(&zip, &[("A.pak", b"new")]);
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();
        fs::write(out.join("A.pak"), b"old").unwrap();

        assert!(ZipArchiveReader::default()
            .extract_flat(&zip, &out)
            .is_err());
        assert_eq!(fs::read(out.join("A.pak")).unwrap(), b"old");
    }

    #[test]
    fn invalid_archive_is_corrupted_and_missing_archive_is_a_read_error() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.zip");
        fs::write(&bad, b"this is not a zip").unwrap();
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        assert!(matches!(
            ZipArchiveReader::default().extract_flat(&bad, &out),
            Err(LibraryError::Storage(AppError::Corrupted { .. }))
        ));
        assert!(matches!(
            ZipArchiveReader::default().extract_flat(&dir.path().join("missing.zip"), &out),
            Err(LibraryError::Storage(AppError::Read { .. }))
        ));
    }

    fn limited(max_entry_bytes: u64, max_total_bytes: u64, max_entries: usize) -> ZipArchiveReader {
        ZipArchiveReader::with_limits(ZipLimits {
            max_entry_bytes,
            max_total_bytes,
            max_entries,
        })
    }

    #[test]
    fn default_limits_are_generous_but_finite() {
        let limits = ZipLimits::default();
        assert_eq!(limits.max_entry_bytes, 8 * 1024 * 1024 * 1024);
        assert_eq!(limits.max_total_bytes, 16 * 1024 * 1024 * 1024);
        assert_eq!(limits.max_entries, 10_000);
    }

    #[test]
    fn entry_over_the_per_entry_cap_is_rejected_while_streaming() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("big.zip");
        build_zip(&zip, &[("A.pak", &[1u8; 33])]);
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        let error = limited(32, 1024, 10).extract_flat(&zip, &out).unwrap_err();

        assert_eq!(
            error,
            LibraryError::ArchiveLimitExceeded {
                limit: ArchiveLimit::EntryBytes
            }
        );
        // Exactly at the cap is fine.
        let ok = dir.path().join("ok");
        fs::create_dir(&ok).unwrap();
        assert!(limited(33, 1024, 10).extract_flat(&zip, &ok).is_ok());
    }

    #[test]
    fn total_extracted_bytes_over_the_cap_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("many.zip");
        build_zip(&zip, &[("A.pak", &[1u8; 20]), ("B.pak", &[2u8; 20])]);
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        let error = limited(32, 39, 10).extract_flat(&zip, &out).unwrap_err();

        assert_eq!(
            error,
            LibraryError::ArchiveLimitExceeded {
                limit: ArchiveLimit::TotalBytes
            }
        );
    }

    #[test]
    fn archive_with_too_many_entries_is_rejected_before_extracting() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("count.zip");
        build_zip(&zip, &[("A.pak", b"a"), ("B.pak", b"b"), ("c.txt", b"c")]);
        let out = dir.path().join("out");
        fs::create_dir(&out).unwrap();

        let error = limited(32, 1024, 2).extract_flat(&zip, &out).unwrap_err();

        assert_eq!(
            error,
            LibraryError::ArchiveLimitExceeded {
                limit: ArchiveLimit::EntryCount
            }
        );
        assert!(fs::read_dir(&out).unwrap().next().is_none());
    }

    #[test]
    fn unwritable_destination_is_a_write_error() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mods.zip");
        build_zip(&zip, &[("A.pak", b"a")]);
        let missing_dest = dir.path().join("does-not-exist");
        assert!(matches!(
            ZipArchiveReader::default().extract_flat(&zip, &missing_dest),
            Err(LibraryError::Storage(AppError::Write { .. }))
        ));
    }
}
