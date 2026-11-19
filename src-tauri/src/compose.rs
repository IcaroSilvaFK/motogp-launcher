//! Composition of the use cases with the real adapters. Everything that
//! varies between production and tests (Steam, opener, process table,
//! background execution, events) is injected.
use motogp_application::{
    BackgroundRunner, DeployService, GameService, GameSettings, PlayService, ProcessInspector,
    ProfileModPlanner, ProfileService, SessionPublisher, SessionService, UrlOpener,
};
use motogp_infrastructure::{
    mod_library, AtomicSessionLock, DataDir, FsDeployFiles, FsGameFiles, JsonConfigStore,
    JsonDeployedRepository, JsonModRepository, JsonProfileStore, ModLibrary, MonotonicClock,
    SteamLibraries, SystemClock,
};
use std::sync::Arc;

pub type Profiles = ProfileService<JsonProfileStore, SystemClock>;
pub type Settings = GameSettings<JsonConfigStore, SteamLibraries, FsGameFiles>;

/// Everything the commands need. Managed as Tauri state.
pub struct AppState {
    pub settings: Arc<Settings>,
    pub profiles: Arc<Profiles>,
    pub library: ModLibrary,
    /// For use cases that update the configuration next to the profiles.
    pub config: JsonConfigStore,
    pub play: PlayService,
}

pub fn compose<O, P>(
    data: &DataDir,
    steam: SteamLibraries,
    opener: O,
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
        library: mod_library(data),
        config: JsonConfigStore::new(data.config_file()),
        play,
    }
}
