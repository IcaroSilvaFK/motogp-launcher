//! JSON persistence adapter: generic `load_json`/`save_json` plus the
//! `ConfigRepository` implementation backed by `config.json`.
use motogp_application::{AppError, ConfigRepository};
use motogp_domain::Config;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Serialization shape of `config.json` (kept out of the domain model).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ConfigDto {
    game_dir: Option<PathBuf>,
    active_profile: Option<String>,
}

impl From<ConfigDto> for Config {
    fn from(dto: ConfigDto) -> Self {
        Config {
            game_dir: dto.game_dir,
            active_profile: dto.active_profile,
        }
    }
}

impl From<&Config> for ConfigDto {
    fn from(config: &Config) -> Self {
        ConfigDto {
            game_dir: config.game_dir.clone(),
            active_profile: config.active_profile.clone(),
        }
    }
}

/// Loads JSON from `path`. A missing file yields `T::default()`; an existing
/// but empty or invalid file is an error (and is never overwritten).
pub fn load_json<T: DeserializeOwned + Default>(_path: &Path) -> Result<T, AppError> {
    todo!("red")
}

/// Saves `value` as pretty JSON through `<path>.tmp` and an atomic replace.
/// On failure the previous file is left untouched and the temp file removed.
pub fn save_json<T: Serialize>(_path: &Path, _value: &T) -> Result<(), AppError> {
    todo!("red")
}

/// `ConfigRepository` backed by a JSON file.
#[derive(Debug, Clone)]
pub struct JsonConfigStore {
    path: PathBuf,
}

impl JsonConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ConfigRepository for JsonConfigStore {
    fn load(&self) -> Result<Config, AppError> {
        load_json::<ConfigDto>(&self.path).map(Config::from)
    }

    fn save(&self, config: &Config) -> Result<(), AppError> {
        save_json(&self.path, &ConfigDto::from(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn sample() -> Config {
        Config {
            game_dir: Some(PathBuf::from("Program Files (x86)").join("MotoGP")),
            active_profile: Some("p1".to_string()),
        }
    }

    fn tmp_of(path: &Path) -> PathBuf {
        let mut os = path.as_os_str().to_owned();
        os.push(".tmp");
        PathBuf::from(os)
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let store = JsonConfigStore::new(dir.path().join("config.json"));
        assert_eq!(store.load(), Ok(Config::default()));
    }

    #[test]
    fn roundtrip_in_path_with_accents() {
        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join("Usuário Padrão")
            .join("dados")
            .join("config.json");
        let store = JsonConfigStore::new(&path);
        store.save(&sample()).unwrap();
        assert!(path.parent().unwrap().is_dir());
        assert_eq!(store.load(), Ok(sample()));
        assert!(!tmp_of(&path).exists());
        assert!(
            fs::read_to_string(&path).unwrap().contains('\n'),
            "pretty-printed"
        );
    }

    #[test]
    fn corrupted_json_is_an_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"{ not json").unwrap();
        let store = JsonConfigStore::new(&path);
        assert!(matches!(store.load(), Err(AppError::Corrupted { .. })));
        assert_eq!(fs::read(&path).unwrap(), b"{ not json");
    }

    #[test]
    fn zero_byte_file_is_an_error_not_a_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"").unwrap();
        let store = JsonConfigStore::new(&path);
        assert!(matches!(store.load(), Err(AppError::Corrupted { .. })));
        assert_eq!(fs::read(&path).unwrap(), b"");
    }

    #[test]
    fn save_replaces_existing_file_atomically() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = JsonConfigStore::new(&path);
        store.save(&sample()).unwrap();
        let second = Config {
            game_dir: None,
            active_profile: Some("p2".to_string()),
        };
        store.save(&second).unwrap();
        assert_eq!(store.load(), Ok(second));
        assert!(!tmp_of(&path).exists());
    }

    #[test]
    fn failed_save_preserves_previous_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = JsonConfigStore::new(&path);
        store.save(&sample()).unwrap();
        let before = fs::read(&path).unwrap();
        // A directory squatting on the temp path makes the temp write fail
        // deterministically on every platform.
        fs::create_dir(tmp_of(&path)).unwrap();
        let result = store.save(&Config::default());
        assert!(matches!(result, Err(AppError::Write { .. })));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(store.load(), Ok(sample()));
    }

    #[test]
    fn failed_replace_removes_the_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // A directory as destination makes the final replace fail.
        fs::create_dir(&path).unwrap();
        let result = save_json(&path, &ConfigDto::default());
        assert!(matches!(result, Err(AppError::Write { .. })));
        assert!(path.is_dir());
        assert!(!tmp_of(&path).exists());
    }

    #[test]
    fn unreadable_path_is_a_read_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::create_dir(&path).unwrap();
        assert!(matches!(
            load_json::<ConfigDto>(&path),
            Err(AppError::Read { .. })
        ));
    }
}
