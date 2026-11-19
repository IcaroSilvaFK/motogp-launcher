//! JSON persistence adapter: generic `load_json`/`save_json` plus the
//! `ConfigRepository` implementation backed by `config.json`.
use motogp_application::{AppError, ConfigRepository};
use motogp_domain::Config;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T, AppError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(T::default()),
        Err(e) => {
            return Err(AppError::Read {
                location: path.display().to_string(),
                detail: e.to_string(),
            })
        }
    };
    serde_json::from_slice(&bytes).map_err(|e| AppError::Corrupted {
        location: path.display().to_string(),
        detail: e.to_string(),
    })
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique per invocation (pid + counter), in the same directory as `path`.
fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut os = path.as_os_str().to_owned();
    os.push(format!(".{}-{}.tmp", std::process::id(), n));
    PathBuf::from(os)
}

fn write_error(path: &Path, detail: impl ToString) -> AppError {
    AppError::Write {
        location: path.display().to_string(),
        detail: detail.to_string(),
    }
}

/// Saves `value` as pretty JSON through a unique `<path>.<pid>-<n>.tmp` and an atomic replace.
/// On failure the previous file is left untouched and the temp file removed.
///
/// `fs::rename` replaces an existing destination on Windows too (std uses
/// `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`, or POSIX-semantics rename
/// on recent versions), so no extra crate is needed.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| write_error(path, e))?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| write_error(path, e))?;
    }
    // `create_new` never opens (or truncates) a file made by another writer;
    // on a name collision we pick a fresh name.
    let (tmp, mut file) = loop {
        let tmp = temp_path(path);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => break (tmp, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(write_error(path, e)),
        }
    };
    let result = file
        .write_all(&json)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&tmp, path)
        });
    if let Err(e) = result {
        // Only the temp file created by this invocation is removed.
        let _ = fs::remove_file(&tmp);
        return Err(write_error(path, e));
    }
    Ok(())
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

    fn temp_files_in(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "tmp"))
            .collect()
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
        assert!(temp_files_in(path.parent().unwrap()).is_empty());
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
        assert!(temp_files_in(dir.path()).is_empty());
    }

    #[test]
    fn save_never_touches_a_competing_writers_temp_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = JsonConfigStore::new(&path);
        let competitors = [
            dir.path().join("config.json.tmp"),
            dir.path().join("config.json.4242-0.tmp"),
        ];
        for c in &competitors {
            fs::write(c, b"in-flight bytes of another writer").unwrap();
        }
        store.save(&sample()).unwrap();
        assert_eq!(store.load(), Ok(sample()));
        for c in &competitors {
            assert_eq!(fs::read(c).unwrap(), b"in-flight bytes of another writer");
        }
    }

    #[cfg(windows)]
    #[test]
    fn failed_replace_over_a_locked_destination_preserves_file_and_cleans_up() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let store = JsonConfigStore::new(&path);
        store.save(&sample()).unwrap();
        let before = fs::read(&path).unwrap();
        // Open without FILE_SHARE_DELETE (0x4): replacing the file must fail.
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(&path)
            .unwrap();
        let result = store.save(&Config::default());
        assert!(matches!(result, Err(AppError::Write { .. })), "{result:?}");
        drop(lock);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(store.load(), Ok(sample()));
        assert!(temp_files_in(dir.path()).is_empty());
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
        assert!(temp_files_in(dir.path()).is_empty());
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
