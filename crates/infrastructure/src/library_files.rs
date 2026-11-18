//! `LibraryFiles` adapter: the `library/` folder on disk. IDs and file names
//! are validated here (the files boundary), so nothing is written or deleted
//! outside the library.
use motogp_application::{AppError, LibraryError, LibraryFiles};
use motogp_domain::is_valid_mod_id;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const STAGING_PREFIX: &str = ".staging-";
static STAGING_COUNTER: AtomicU32 = AtomicU32::new(0);

fn read_error(path: &Path, error: impl ToString) -> LibraryError {
    LibraryError::Storage(AppError::Read {
        location: path.display().to_string(),
        detail: error.to_string(),
    })
}

fn write_error(path: &Path, error: impl ToString) -> LibraryError {
    LibraryError::Storage(AppError::Write {
        location: path.display().to_string(),
        detail: error.to_string(),
    })
}

/// A plain file name: exactly one normal path component, no separators.
fn is_plain_file_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(part)), None) if part == name && !name.contains(['/', '\\'])
    )
}

/// File operations rooted at the library directory.
#[derive(Debug, Clone)]
pub struct FsLibraryFiles {
    library_dir: PathBuf,
}

impl FsLibraryFiles {
    pub fn new(library_dir: impl Into<PathBuf>) -> Self {
        Self {
            library_dir: library_dir.into(),
        }
    }
}

impl FsLibraryFiles {
    /// Only direct children of the library named `.staging-*` may be moved
    /// from or deleted as staging areas.
    fn check_staging(&self, staging: &Path) -> Result<(), LibraryError> {
        let is_staging = staging.parent() == Some(self.library_dir.as_path())
            && staging
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(STAGING_PREFIX));
        if is_staging {
            Ok(())
        } else {
            Err(write_error(
                staging,
                "not a staging directory of the library",
            ))
        }
    }
}

impl LibraryFiles for FsLibraryFiles {
    fn list_dir(&self, dir: &Path) -> Result<Vec<String>, LibraryError> {
        let mut names = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| read_error(dir, e))? {
            let entry = entry.map_err(|e| read_error(dir, e))?;
            if entry.file_type().map_err(|e| read_error(dir, e))?.is_file() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        Ok(names)
    }

    fn create_staging(&self) -> Result<PathBuf, LibraryError> {
        fs::create_dir_all(&self.library_dir).map_err(|e| write_error(&self.library_dir, e))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let sequence = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let staging = self.library_dir.join(format!(
            "{STAGING_PREFIX}{}-{nanos}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&staging).map_err(|e| write_error(&staging, e))?;
        Ok(staging)
    }

    fn copy_file(&self, source: &Path, dest_dir: &Path) -> Result<(), LibraryError> {
        let name = source
            .file_name()
            .ok_or_else(|| read_error(source, "not a file path"))?;
        let target = dest_dir.join(name);
        let mut input = fs::File::open(source).map_err(|e| read_error(source, e))?;
        // `create_new` never overwrites an existing file.
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| write_error(&target, e))?;
        io::copy(&mut input, &mut output).map_err(|e| write_error(&target, e))?;
        Ok(())
    }

    fn file_size(&self, file: &Path) -> Result<u64, LibraryError> {
        fs::metadata(file)
            .map(|m| m.len())
            .map_err(|e| read_error(file, e))
    }

    fn publish(&self, staging: &Path, files: &[String], id: &str) -> Result<(), LibraryError> {
        if !is_valid_mod_id(id) {
            return Err(LibraryError::InvalidId { id: id.to_string() });
        }
        self.check_staging(staging)?;
        if let Some(bad) = files.iter().find(|name| !is_plain_file_name(name)) {
            return Err(write_error(staging, format!("unsafe file name {bad:?}")));
        }

        let target = self.library_dir.join(id);
        let existed = target.exists();
        fs::create_dir_all(&target).map_err(|e| write_error(&target, e))?;
        for name in files {
            if let Err(e) = fs::rename(staging.join(name), target.join(name)) {
                if !existed {
                    let _ = fs::remove_dir_all(&target);
                }
                return Err(write_error(&target.join(name), e));
            }
        }
        Ok(())
    }

    fn discard(&self, staging: &Path) -> Result<(), LibraryError> {
        self.check_staging(staging)?;
        match fs::remove_dir_all(staging) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(write_error(staging, e)),
            _ => Ok(()),
        }
    }

    fn remove_mod_dir(&self, id: &str) -> Result<(), LibraryError> {
        if !is_valid_mod_id(id) {
            return Err(LibraryError::InvalidId { id: id.to_string() });
        }
        let target = self.library_dir.join(id);
        match fs::remove_dir_all(&target) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(write_error(&target, e)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ID: &str = "0123456789ab";

    fn setup() -> (tempfile::TempDir, FsLibraryFiles, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("Usuário Padrão").join("library");
        (dir, FsLibraryFiles::new(&library), library)
    }

    #[test]
    fn staging_directories_are_unique_and_inside_the_library() {
        let (_guard, files, library) = setup();
        let a = files.create_staging().unwrap();
        let b = files.create_staging().unwrap();
        assert_ne!(a, b);
        assert!(a.is_dir() && b.is_dir());
        assert_eq!(a.parent(), Some(library.as_path()));
    }

    #[test]
    fn copy_file_copies_into_the_destination_and_keeps_the_name() {
        let (guard, files, _library) = setup();
        let source = guard.path().join("A_P.pak");
        fs::write(&source, b"data").unwrap();
        let staging = files.create_staging().unwrap();
        files.copy_file(&source, &staging).unwrap();
        assert_eq!(fs::read(staging.join("A_P.pak")).unwrap(), b"data");
        assert_eq!(files.file_size(&staging.join("A_P.pak")).unwrap(), 4);
        // Never overwrites an existing file.
        assert!(files.copy_file(&source, &staging).is_err());
        assert!(files
            .copy_file(&guard.path().join("missing.pak"), &staging)
            .is_err());
    }

    #[test]
    fn list_dir_returns_only_file_names() {
        let (guard, files, _library) = setup();
        fs::write(guard.path().join("A.pak"), b"1").unwrap();
        fs::write(guard.path().join("B.utoc"), b"2").unwrap();
        fs::create_dir(guard.path().join("sub")).unwrap();
        let mut names = files.list_dir(guard.path()).unwrap();
        names.sort();
        assert_eq!(names, vec!["A.pak", "B.utoc"]);
        assert!(files.list_dir(&guard.path().join("missing")).is_err());
    }

    #[test]
    fn file_size_of_a_missing_file_is_an_error() {
        let (guard, files, _library) = setup();
        assert!(files.file_size(&guard.path().join("missing")).is_err());
    }

    #[test]
    fn publish_moves_the_named_files_into_the_id_folder() {
        let (_guard, files, library) = setup();
        let staging = files.create_staging().unwrap();
        fs::write(staging.join("A.pak"), b"1").unwrap();
        fs::write(staging.join("A.utoc"), b"2").unwrap();
        files
            .publish(&staging, &["A.pak".into(), "A.utoc".into()], ID)
            .unwrap();
        assert_eq!(fs::read(library.join(ID).join("A.pak")).unwrap(), b"1");
        assert_eq!(fs::read(library.join(ID).join("A.utoc")).unwrap(), b"2");
        assert!(!staging.join("A.pak").exists());
    }

    #[test]
    fn publish_rejects_unsafe_ids_and_names() {
        let (_guard, files, library) = setup();
        let staging = files.create_staging().unwrap();
        fs::write(staging.join("A.pak"), b"1").unwrap();
        assert!(files
            .publish(&staging, &["A.pak".into()], "../evil")
            .is_err());
        assert!(files.publish(&staging, &["A.pak".into()], "short").is_err());
        for name in ["../A.pak", "x/A.pak", "x\\A.pak", "..", ""] {
            assert!(
                files.publish(&staging, &[name.into()], ID).is_err(),
                "{name:?} should be rejected"
            );
        }
        assert!(!library.join(ID).exists());
        assert!(staging.join("A.pak").exists());
    }

    #[test]
    fn failed_publish_removes_the_folder_it_created() {
        let (_guard, files, library) = setup();
        let staging = files.create_staging().unwrap();
        fs::write(staging.join("A.pak"), b"1").unwrap();
        let result = files.publish(&staging, &["A.pak".into(), "missing.utoc".into()], ID);
        assert!(result.is_err());
        assert!(!library.join(ID).exists());
    }

    #[test]
    fn discard_removes_staging_but_never_paths_outside_the_library() {
        let (guard, files, library) = setup();
        let staging = files.create_staging().unwrap();
        fs::write(staging.join("A.pak"), b"1").unwrap();
        files.discard(&staging).unwrap();
        assert!(!staging.exists());
        // Discarding something already gone is fine.
        files.discard(&staging).unwrap();

        let outside = guard.path().join("outside");
        fs::create_dir(&outside).unwrap();
        assert!(files.discard(&outside).is_err());
        assert!(files.discard(&library).is_err());
        assert!(files.discard(&library.join("..").join("outside")).is_err());
        assert!(outside.exists());
    }

    #[test]
    fn remove_mod_dir_deletes_only_the_validated_id_folder() {
        let (_guard, files, library) = setup();
        let staging = files.create_staging().unwrap();
        fs::write(staging.join("A.pak"), b"1").unwrap();
        files.publish(&staging, &["A.pak".into()], ID).unwrap();
        fs::create_dir_all(library.join("keep")).unwrap();

        files.remove_mod_dir(ID).unwrap();
        files.remove_mod_dir(ID).unwrap(); // already gone

        assert!(!library.join(ID).exists());
        assert!(library.join("keep").exists());
        assert!(files.remove_mod_dir("..").is_err());
        assert!(files.remove_mod_dir("../library").is_err());
        assert!(library.exists());
    }
}
