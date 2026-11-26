//! The random install id of the anonymous download counts (issue #120): 32
//! bytes from the OS CSPRNG as unpadded base64url (43 characters), kept in the
//! `install-id` file of the data directory. It is not personal data and is
//! never logged.
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use motogp_application::{DownloadStatsError, InstallIdStore};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

const ID_BYTES: usize = 32;
/// Length of 32 bytes in unpadded base64url.
const ID_LEN: usize = 43;

/// [`InstallIdStore`] backed by one small file.
#[derive(Debug)]
pub struct FsInstallIdStore {
    path: PathBuf,
    /// Two threads asking at once must end up with the same id.
    guard: Mutex<()>,
}

impl FsInstallIdStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            guard: Mutex::new(()),
        }
    }
}

fn is_well_formed(id: &str) -> bool {
    id.len() == ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn fresh_id() -> Result<String, DownloadStatsError> {
    let mut bytes = [0u8; ID_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|_| DownloadStatsError("the random generator failed".to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

impl InstallIdStore for FsInstallIdStore {
    fn load_or_create(&self) -> Result<String, DownloadStatsError> {
        let _one_at_a_time = self.guard.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(text) = fs::read_to_string(&self.path) {
            let id = text.trim();
            if is_well_formed(id) {
                return Ok(id.to_string());
            }
        }
        let id = fresh_id()?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|_| DownloadStatsError("could not save the install id".to_string()))?;
        }
        fs::write(&self.path, &id)
            .map_err(|_| DownloadStatsError("could not save the install id".to_string()))?;
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn store_in(dir: &tempfile::TempDir) -> FsInstallIdStore {
        FsInstallIdStore::new(dir.path().join("install-id"))
    }

    #[test]
    fn a_missing_file_creates_a_43_character_url_safe_id_and_saves_it() {
        let dir = tempfile::tempdir().unwrap();
        let id = store_in(&dir).load_or_create().unwrap();
        assert_eq!(id.len(), 43);
        assert!(is_well_formed(&id));
        assert_eq!(
            fs::read_to_string(dir.path().join("install-id")).unwrap(),
            id
        );
    }

    #[test]
    fn the_same_id_comes_back_on_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let first = store_in(&dir).load_or_create().unwrap();
        assert_eq!(store_in(&dir).load_or_create().unwrap(), first);
    }

    #[test]
    fn a_trailing_newline_is_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let id = "A".repeat(43);
        fs::write(dir.path().join("install-id"), format!("{id}\n")).unwrap();
        assert_eq!(store_in(&dir).load_or_create().unwrap(), id);
    }

    #[test]
    fn a_malformed_file_is_regenerated() {
        let dir = tempfile::tempdir().unwrap();
        for bad in [
            "",
            "short",
            &"A".repeat(44),
            &format!("{}!", "A".repeat(42)),
        ] {
            fs::write(dir.path().join("install-id"), bad).unwrap();
            let id = store_in(&dir).load_or_create().unwrap();
            assert!(is_well_formed(&id), "{bad:?}");
            assert_eq!(
                fs::read_to_string(dir.path().join("install-id")).unwrap(),
                id
            );
        }
    }

    #[test]
    fn two_installs_get_different_ids() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert_ne!(
            store_in(&a).load_or_create().unwrap(),
            store_in(&b).load_or_create().unwrap()
        );
    }

    #[test]
    fn a_non_ascii_data_folder_works_and_is_created_when_missing() {
        // Windows folder `Usuário Padrão` on purpose: real users have accented paths.
        let dir = tempfile::tempdir().unwrap();
        let path = dir
            .path()
            .join("Usuário Padrão")
            .join("data")
            .join("install-id");
        let id = FsInstallIdStore::new(&path).load_or_create().unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), id);
    }

    #[test]
    fn an_unwritable_location_is_an_error_without_the_id() {
        let dir = tempfile::tempdir().unwrap();
        // The file path is an existing directory: it cannot be written.
        let path = dir.path().join("install-id");
        fs::create_dir(&path).unwrap();
        let error = FsInstallIdStore::new(&path).load_or_create().unwrap_err();
        assert_eq!(error.0, "could not save the install id");
    }

    #[test]
    fn concurrent_first_launches_agree_on_one_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(store_in(&dir));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let store = store.clone();
                thread::spawn(move || store.load_or_create().unwrap())
            })
            .collect();
        let ids: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(ids.windows(2).all(|pair| pair[0] == pair[1]));
    }
}
