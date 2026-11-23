//! Composition of the use cases with the real adapters. Everything that
//! varies between production and tests (Steam, opener, process table,
//! background execution, events) is injected.
use motogp_application::{
    BackgroundRunner, DeployService, FolderOpener, GameService, GameSettings, PlayService,
    ProcessInspector, ProfileModPlanner, ProfileService, SessionPublisher, SessionService,
    StorageService, UrlOpener,
};
use motogp_infrastructure::{
    mod_library, AtomicSessionLock, DataDir, FsDeployFiles, FsGameFiles, FsStorageScanner,
    JsonConfigStore, JsonDeployedRepository, JsonModRepository, JsonProfileStore, ModLibrary,
    MonotonicClock, SteamLibraries, SystemClock,
};
use std::sync::{Arc, Mutex};

pub type Profiles = ProfileService<JsonProfileStore, SystemClock>;
pub type Settings = GameSettings<JsonConfigStore, SteamLibraries, FsGameFiles>;
pub type Storage = StorageService<FsStorageScanner, Arc<dyn FolderOpener + Send + Sync>>;

/// Everything the commands need. Managed as Tauri state; cheap to clone so
/// blocking use cases can run off the main thread.
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub profiles: Arc<Profiles>,
    pub library: Arc<ModLibrary>,
    /// For use cases that update the configuration next to the profiles.
    pub config: JsonConfigStore,
    pub play: PlayService,
    /// Read-only: measuring never takes `write_lock`.
    pub storage: Arc<Storage>,
    /// Serializes every mutation of the shared JSON stores, so concurrent
    /// commands cannot lose each other's updates.
    pub write_lock: Arc<Mutex<()>>,
}

pub fn compose<O, P>(
    data: &DataDir,
    steam: SteamLibraries,
    opener: O,
    folder_opener: Arc<dyn FolderOpener + Send + Sync>,
    inspector: P,
    publisher: Arc<dyn SessionPublisher>,
    runner: Arc<dyn BackgroundRunner>,
) -> AppState
where
    O: UrlOpener + Send + Sync + 'static,
    P: ProcessInspector + Send + Sync + 'static,
{
    let settings = Arc::new(GameSettings::new(
        JsonConfigStore::new(data.config_file()),
        GameService::new(steam, FsGameFiles),
    ));
    let profiles = Arc::new(ProfileService::new(
        JsonProfileStore::new(data.profiles_file()),
        SystemClock,
    ));
    let planner = ProfileModPlanner::new(
        profiles.clone(),
        JsonModRepository::new(data.library_file()),
        data.library_dir(),
    );
    let deployer = DeployService::new(
        JsonDeployedRepository::new(data.deployed_file()),
        FsDeployFiles::new(),
    );
    let runtime = SessionService::new(opener, inspector, MonotonicClock::new());
    let play = PlayService::new(
        settings.clone(),
        Arc::new(planner),
        Arc::new(deployer),
        Arc::new(runtime),
        Arc::new(AtomicSessionLock::default()),
        runner,
        publisher,
    );
    AppState {
        settings,
        profiles,
        library: Arc::new(mod_library(data)),
        write_lock: Arc::default(),
        config: JsonConfigStore::new(data.config_file()),
        play,
        storage: Arc::new(StorageService::new(
            data.root().to_path_buf(),
            FsStorageScanner,
            folder_opener,
        )),
    }
}
