//! `ModRepository` adapter: the library index stored in `library.json`.
use motogp_application::{LibraryError, ModRepository};
use motogp_domain::ModEntry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Serialization shape of one index entry (kept out of the domain model).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModDto {
    id: String,
    name: String,
    files: Vec<String>,
    size_bytes: u64,
}

/// Serialization shape of `library.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct LibraryDto {
    mods: Vec<ModDto>,
}

/// Library index backed by a JSON file (atomic saves via the issue #2 store).
#[derive(Debug, Clone)]
pub struct JsonModRepository {
    path: PathBuf,
}

impl JsonModRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ModRepository for JsonModRepository {
    fn list(&self) -> Result<Vec<ModEntry>, LibraryError> {
        let _ = &self.path;
        todo!()
    }

    fn add(&self, _entry: &ModEntry) -> Result<(), LibraryError> {
        todo!()
    }

    fn remove(&self, _id: &str) -> Result<bool, LibraryError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;
    use std::fs;

    fn entry(id: &str, name: &str) -> ModEntry {
        ModEntry {
            id: id.into(),
            name: name.into(),
            files: vec![format!("{name}_P.pak")],
            size_bytes: 42,
        }
    }

    #[test]
    fn missing_index_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonModRepository::new(dir.path().join("library.json"));
        assert_eq!(repo.list(), Ok(vec![]));
    }

    #[test]
    fn add_then_list_roundtrips_in_a_path_with_accents() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonModRepository::new(dir.path().join("Usuário Padrão").join("library.json"));
        repo.add(&entry("aaaaaaaaaaaa", "A")).unwrap();
        repo.add(&entry("bbbbbbbbbbbb", "B")).unwrap();
        assert_eq!(
            repo.list().unwrap(),
            vec![entry("aaaaaaaaaaaa", "A"), entry("bbbbbbbbbbbb", "B")]
        );
    }

    #[test]
    fn remove_deletes_only_the_requested_entry() {
        let dir = tempfile::tempdir().unwrap();
        let repo = JsonModRepository::new(dir.path().join("library.json"));
        repo.add(&entry("aaaaaaaaaaaa", "A")).unwrap();
        repo.add(&entry("bbbbbbbbbbbb", "B")).unwrap();
        assert_eq!(repo.remove("aaaaaaaaaaaa"), Ok(true));
        assert_eq!(repo.remove("aaaaaaaaaaaa"), Ok(false));
        assert_eq!(repo.list().unwrap(), vec![entry("bbbbbbbbbbbb", "B")]);
    }

    #[test]
    fn remove_of_an_unknown_id_does_not_create_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        assert_eq!(
            JsonModRepository::new(&path).remove("cccccccccccc"),
            Ok(false)
        );
        assert!(!path.exists());
    }

    #[test]
    fn corrupted_index_is_an_error_and_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.json");
        fs::write(&path, "{ not json").unwrap();
        let repo = JsonModRepository::new(&path);
        assert!(matches!(
            repo.list(),
            Err(LibraryError::Storage(AppError::Corrupted { .. }))
        ));
        assert!(repo.add(&entry("aaaaaaaaaaaa", "A")).is_err());
        assert!(repo.remove("aaaaaaaaaaaa").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }
}
