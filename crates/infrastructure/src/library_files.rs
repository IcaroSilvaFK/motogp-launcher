//! `LibraryFiles` adapter: the `library/` folder on disk. IDs and file names
//! are validated here (the files boundary), so nothing is written or deleted
//! outside the library.
use motogp_application::{LibraryError, LibraryFiles};
use std::path::{Path, PathBuf};

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

impl LibraryFiles for FsLibraryFiles {
    fn list_dir(&self, _dir: &Path) -> Result<Vec<String>, LibraryError> {
        let _ = &self.library_dir;
        todo!()
    }

    fn create_staging(&self) -> Result<PathBuf, LibraryError> {
        todo!()
    }

    fn copy_file(&self, _source: &Path, _dest_dir: &Path) -> Result<(), LibraryError> {
        todo!()
    }

    fn file_size(&self, _file: &Path) -> Result<u64, LibraryError> {
        todo!()
    }

    fn publish(&self, _staging: &Path, _files: &[String], _id: &str) -> Result<(), LibraryError> {
        todo!()
    }

    fn discard(&self, _staging: &Path) -> Result<(), LibraryError> {
        todo!()
    }

    fn remove_mod_dir(&self, _id: &str) -> Result<(), LibraryError> {
        todo!()
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
