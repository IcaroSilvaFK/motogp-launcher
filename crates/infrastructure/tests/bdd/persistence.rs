//! Steps for `tests/features/persistence.feature` (issue #2).
//! Only temporary directories are used. Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, ConfigRepository};
use motogp_domain::Config;
use motogp_infrastructure::{DataDir, JsonConfigStore};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug, Default, World)]
struct PersistenceWorld {
    tmp: Option<TempDir>,
    config_path: Option<PathBuf>,
    config: Option<Config>,
    second: Option<Config>,
    loaded: Option<Result<Config, AppError>>,
    save_result: Option<Result<(), AppError>>,
    bytes_before: Option<Vec<u8>>,
}

impl PersistenceWorld {
    fn base(&mut self) -> PathBuf {
        self.tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
            .path()
            .to_path_buf()
    }

    fn path(&self) -> PathBuf {
        self.config_path.clone().expect("config path not set")
    }

    fn store(&self) -> JsonConfigStore {
        JsonConfigStore::new(self.path())
    }

    fn use_config_in(&mut self, dir: PathBuf) {
        self.config_path = Some(DataDir::new(dir).config_file());
    }
}

fn no_temp_files(path: &Path) -> bool {
    let dir = path.parent().unwrap();
    fs::read_dir(dir)
        .unwrap()
        .all(|e| e.unwrap().path().extension().map_or(true, |x| x != "tmp"))
}

fn sample(profile: &str) -> Config {
    Config {
        game_dir: Some(PathBuf::from("Program Files (x86)")),
        active_profile: Some(profile.to_string()),
    }
}

// ---------- Scenario: Read a missing configuration ----------

#[given("config.json does not exist yet")]
fn config_missing(w: &mut PersistenceWorld) {
    let base = w.base();
    w.use_config_in(base);
    assert!(!w.path().exists());
}

#[when("I load the configuration")]
fn load_configuration(w: &mut PersistenceWorld) {
    w.loaded = Some(w.store().load());
}

#[then("I receive the default configuration")]
fn receive_default(w: &mut PersistenceWorld) {
    assert_eq!(w.loaded, Some(Ok(Config::default())));
}

// ---------- Scenario: Save and read in a path with spaces and accents ----------

#[given(expr = "a directory named {string}")]
fn directory_named(w: &mut PersistenceWorld, name: String) {
    let dir = w.base().join(name).join("dados");
    w.use_config_in(dir);
}

#[given(expr = "a configuration with game_dir at {string} and active_profile {string}")]
fn configuration_with(w: &mut PersistenceWorld, game_dir: String, profile: String) {
    w.config = Some(Config {
        game_dir: Some(PathBuf::from(game_dir)),
        active_profile: Some(profile),
    });
}

#[when("I save and read config.json")]
fn save_and_read(w: &mut PersistenceWorld) {
    let config = w.config.clone().expect("config not set");
    let store = w.store();
    w.save_result = Some(store.save(&config));
    w.loaded = Some(store.load());
}

#[then("the values return unchanged")]
fn values_unchanged(w: &mut PersistenceWorld) {
    assert_eq!(w.save_result, Some(Ok(())));
    assert_eq!(w.loaded, Some(Ok(w.config.clone().unwrap())));
}

#[then("the parent directories are created")]
fn parents_created(w: &mut PersistenceWorld) {
    assert!(w.path().parent().unwrap().is_dir());
}

#[then("no config.json.tmp file remains")]
fn no_tmp_remains(w: &mut PersistenceWorld) {
    assert!(no_temp_files(&w.path()));
}

// ---------- Scenario: Refuse corrupted JSON ----------

#[given(expr = "an existing file containing {string}")]
fn existing_file_containing(w: &mut PersistenceWorld, content: String) {
    let base = w.base();
    w.use_config_in(base);
    fs::write(w.path(), content.as_bytes()).unwrap();
    w.bytes_before = Some(content.into_bytes());
}

#[when("I try to load its data")]
fn try_load(w: &mut PersistenceWorld) {
    w.loaded = Some(w.store().load());
}

#[then("I receive a read error")]
fn receive_read_error(w: &mut PersistenceWorld) {
    assert!(
        matches!(w.loaded, Some(Err(AppError::Corrupted { .. }))),
        "got {:?}",
        w.loaded
    );
}

#[then("the original file remains intact")]
fn original_intact(w: &mut PersistenceWorld) {
    assert_eq!(Some(fs::read(w.path()).unwrap()), w.bytes_before);
}

#[then("no default is saved over it")]
fn no_default_saved(w: &mut PersistenceWorld) {
    // A default would have been serialized as JSON with `game_dir`/`active_profile`.
    let on_disk = String::from_utf8(fs::read(w.path()).unwrap()).unwrap();
    assert!(!on_disk.contains("game_dir") && !on_disk.contains("active_profile"));
    assert!(no_temp_files(&w.path()));
}

// ---------- Scenario: Replace existing data safely ----------

#[given("config.json contains a valid configuration")]
fn contains_valid(w: &mut PersistenceWorld) {
    let base = w.base();
    w.use_config_in(base);
    w.store().save(&sample("first")).unwrap();
}

#[when("I save a second configuration")]
fn save_second(w: &mut PersistenceWorld) {
    let second = sample("second");
    w.save_result = Some(w.store().save(&second));
    w.second = Some(second);
}

#[then("the next read returns the complete second configuration")]
fn next_read_second(w: &mut PersistenceWorld) {
    assert_eq!(w.save_result, Some(Ok(())));
    assert_eq!(w.store().load(), Ok(w.second.clone().unwrap()));
}

#[then("there is no pending temporary file")]
fn no_pending_tmp(w: &mut PersistenceWorld) {
    assert!(no_temp_files(&w.path()));
}

// ---------- Scenario: Preserve the previous version when the write fails ----------

#[given("an already saved valid file")]
fn already_saved(w: &mut PersistenceWorld) {
    let base = w.base();
    w.use_config_in(base);
    w.store().save(&sample("first")).unwrap();
    w.config = Some(sample("first"));
    w.bytes_before = Some(fs::read(w.path()).unwrap());
}

#[when("an I/O failure prevents the atomic replacement")]
fn io_failure(w: &mut PersistenceWorld) {
    // Hold the destination open without delete-sharing so the final replace
    // fails deterministically (Windows); elsewhere make the directory read-only.
    #[cfg(windows)]
    let _guard = {
        use std::os::windows::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(w.path())
            .unwrap()
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = w.path().parent().unwrap().to_path_buf();
        fs::set_permissions(dir, fs::Permissions::from_mode(0o555)).unwrap();
    }
    w.save_result = Some(w.store().save(&sample("second")));
}

#[then("the operation returns an error")]
fn operation_errors(w: &mut PersistenceWorld) {
    assert!(
        matches!(w.save_result, Some(Err(AppError::Write { .. }))),
        "got {:?}",
        w.save_result
    );
}

#[then("the previous file remains intact and readable")]
fn previous_intact(w: &mut PersistenceWorld) {
    assert_eq!(Some(fs::read(w.path()).unwrap()), w.bytes_before);
    assert_eq!(w.store().load(), Ok(w.config.clone().unwrap()));
}

// ---------- Scenario: An existing config with a mixed-separator game_dir is fixed on load ----------

#[given(expr = "a config.json whose game_dir is {string}")]
fn config_with_raw_game_dir(w: &mut PersistenceWorld, game_dir: String) {
    let dir = w.base().join("legacy");
    w.use_config_in(dir);
    let path = w.path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let json = serde_json::json!({ "game_dir": game_dir });
    fs::write(path, json.to_string()).unwrap();
}

#[then(expr = "game_dir is {string}")]
fn game_dir_is(w: &mut PersistenceWorld, expected: String) {
    let config = w.loaded.clone().expect("not loaded").expect("load failed");
    assert_eq!(config.game_dir, Some(PathBuf::from(expected)));
}

/// Runs only `persistence.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = PersistenceWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("persistence.feature"))
        .await;
    !writer.execution_has_failed()
}
