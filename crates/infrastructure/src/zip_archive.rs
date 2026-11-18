//! `ArchiveReader` adapter: ZIP extraction. Entries are flattened, unsafe
//! names (no `enclosed_name()`) are ignored and nothing is overwritten.
use motogp_application::{AppError, ArchiveReader, LibraryError};
use motogp_domain::is_mod_file;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::Path;

/// Characters that are invalid in Windows file names (or control characters).
fn has_invalid_chars(name: &str) -> bool {
    name.chars().any(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/')
    })
}

/// Reads ZIP archives with the `zip` crate.
#[derive(Debug, Default, Clone, Copy)]
pub struct ZipArchiveReader;

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
            io::copy(&mut entry, &mut out).map_err(write_error)?;
            written.push(name);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;
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

        let names = ZipArchiveReader.extract_flat(&zip, &out).unwrap();

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

        let names = ZipArchiveReader.extract_flat(&zip, &out).unwrap();

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

        let error = ZipArchiveReader.extract_flat(&zip, &out).unwrap_err();

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

        assert!(ZipArchiveReader.extract_flat(&zip, &out).is_err());
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
            ZipArchiveReader.extract_flat(&bad, &out),
            Err(LibraryError::Storage(AppError::Corrupted { .. }))
        ));
        assert!(matches!(
            ZipArchiveReader.extract_flat(&dir.path().join("missing.zip"), &out),
            Err(LibraryError::Storage(AppError::Read { .. }))
        ));
    }

    #[test]
    fn unwritable_destination_is_a_write_error() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mods.zip");
        build_zip(&zip, &[("A.pak", b"a")]);
        let missing_dest = dir.path().join("does-not-exist");
        assert!(matches!(
            ZipArchiveReader.extract_flat(&zip, &missing_dest),
            Err(LibraryError::Storage(AppError::Write { .. }))
        ));
    }
}
