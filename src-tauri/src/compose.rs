//! Composition of the use cases with the real adapters. Everything that
//! varies between production and tests (Steam, opener, process table,
//! background execution, events) is injected.
use crate::catalog_install_support::production_downloader;
use crate::comments_fallback::offline_comments;
use crate::{
    MemoryCredentialStore, UnavailableDownloadStats, UnavailableModStatsApi, UnavailablePublishApi,
};
use motogp_application::{
    AdBeaconSink, AdCache, AdService, AdSource, ArtworkService, BackgroundRunner, CancelFlag,
    CatalogInstaller, CatalogService, CatalogSource, CommunityAuthService,
    CommunityCommentsService, CommunityPublishService, CommunityStarsService, DeployService,
    DiagnosticsService, DownloadStatsService, FolderOpener, GameService, GameSettings,
    ModDownloader, PlayService, ProcessInspector, ProfileModPlanner, ProfileService, RawCatalog,
    SessionPublisher, SessionService, StorageService, UrlOpener, WebPageOpener,
};
use motogp_infrastructure::{
    mod_library, AtomicSessionLock, DataDir, FileLog, FsDeployFiles, FsDownloadStaging,
    FsGameFiles, FsInstallIdStore, FsPublishFiles, FsStorageScanner, JsonAdCache, JsonCatalogCache,
    JsonConfigStore, JsonDeployedRepository, JsonModRepository, JsonProfileStore, ModLibrary,
    MonotonicClock, Sha256Hasher, SteamArtwork, SteamLibraries, SysinfoOs, SystemClock,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type Profiles = ProfileService<JsonProfileStore, SystemClock>;
pub type Settings = GameSettings<JsonConfigStore, SteamLibraries, FsGameFiles>;
pub type Diagnostics =
    DiagnosticsService<Arc<FileLog>, SysinfoOs, Arc<dyn FolderOpener + Send + Sync>>;
pub type Storage = StorageService<FsStorageScanner, Arc<dyn FolderOpener + Send + Sync>>;
pub type Catalog = CatalogService<Arc<dyn CatalogSource>, JsonCatalogCache, SystemClock>;
pub type Installer = CatalogInstaller<Arc<dyn ModDownloader>, FsDownloadStaging, Sha256Hasher>;
pub type Ads = AdService<Arc<dyn AdSource>, Arc<dyn AdCache>, Arc<dyn AdBeaconSink>, SystemClock>;

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
    /// Log file, OS facts and the logs folder (#148). Read-only.
    pub diagnostics: Arc<Diagnostics>,
    /// Read-only: reads Steam's local artwork cache.
    pub artwork: Arc<ArtworkService<SteamArtwork>>,
    /// Serializes every mutation of the shared JSON stores, so concurrent
    /// commands cannot lose each other's updates.
    pub write_lock: Arc<Mutex<()>>,
    /// Online catalog (#72). Blocking network I/O: commands run it off the
    /// async runtime and never under `write_lock`.
    pub catalog: Arc<Catalog>,
    /// Opens catalog pages in the default browser (https, allow-listed hosts).
    pub web_opener: Arc<dyn WebPageOpener>,
    /// Opens sponsor click URLs (https only, any host).
    pub ad_opener: Arc<dyn WebPageOpener>,
    /// Sponsored slots (#103). Feed and beacon injected, cache on disk.
    pub ads: Arc<Ads>,
    /// Community account (#122): sign-in with Discord. Blocking network I/O and a
    /// browser wait of up to 2 minutes, so commands never take `write_lock` for it.
    pub community: Arc<CommunityAuthService>,
    /// Mod stars (#119): reads the session from the credential store itself.
    pub stars: Arc<CommunityStarsService>,
    /// Mod comments (#118): same blocking-I/O rules as `community`; the token never leaves it.
    pub comments: Arc<CommunityCommentsService>,
    /// Publishing mods to the community server (#130): reads the session itself.
    pub publish: Arc<CommunityPublishService>,
    /// Cancel flag of the publish in flight (one at a time).
    pub publish_cancel: Arc<Mutex<Option<CancelFlag>>>,
    /// Catalog installs (#74): downloads run off the runtime without `write_lock`.
    pub installer: Arc<Installer>,
    /// Cancel flags of the installs in flight, by catalog entry id.
    pub install_cancels: Arc<Mutex<HashMap<String, CancelFlag>>>,
    /// Anonymous download counts (#120). Blocking network I/O; every failure is "nothing".
    pub downloads: Arc<DownloadStatsService>,
}

/// Serves the bundled snapshot when nothing is cached and the source fails.
fn with_bundled(service: Catalog, bundled: Option<RawCatalog>) -> Catalog {
    match bundled {
        Some(bundled) => service.with_bundled(bundled),
        None => service,
    }
}

// The composition root receives every injected port, so it is wide on purpose.
#[allow(clippy::too_many_arguments)]
pub fn compose<O, P>(
    data: &DataDir,
    steam: SteamLibraries,
    artwork: SteamArtwork,
    opener: O,
    folder_opener: Arc<dyn FolderOpener + Send + Sync>,
    web_opener: Arc<dyn WebPageOpener>,
    ad_opener: Arc<dyn WebPageOpener>,
    inspector: P,
    publisher: Arc<dyn SessionPublisher>,
    runner: Arc<dyn BackgroundRunner>,
    catalog_source: Arc<dyn CatalogSource>,
    ad_source: Arc<dyn AdSource>,
    ad_beacon: Arc<dyn AdBeaconSink>,
    community: Arc<CommunityAuthService>,
    bundled_catalog: Option<RawCatalog>,
) -> AppState
where
    O: UrlOpener + Send + Sync + 'static,
    P: ProcessInspector + Send + Sync + 'static,
{
    compose_with_downloader(
        data,
        steam,
        artwork,
        opener,
        folder_opener,
        web_opener,
        ad_opener,
        inspector,
        publisher,
        runner,
        catalog_source,
        ad_source,
        ad_beacon,
        community,
        bundled_catalog,
        production_downloader(),
    )
}

/// Like [`compose`], with the mod downloader injected (tests use a fake).
// Same wide composition root as `compose`.
#[allow(clippy::too_many_arguments)]
pub fn compose_with_downloader<O, P>(
    data: &DataDir,
    steam: SteamLibraries,
    artwork: SteamArtwork,
    opener: O,
    folder_opener: Arc<dyn FolderOpener + Send + Sync>,
    web_opener: Arc<dyn WebPageOpener>,
    ad_opener: Arc<dyn WebPageOpener>,
    inspector: P,
    publisher: Arc<dyn SessionPublisher>,
    runner: Arc<dyn BackgroundRunner>,
    catalog_source: Arc<dyn CatalogSource>,
    ad_source: Arc<dyn AdSource>,
    ad_beacon: Arc<dyn AdBeaconSink>,
    community: Arc<CommunityAuthService>,
    bundled_catalog: Option<RawCatalog>,
    downloader: Arc<dyn ModDownloader>,
) -> AppState
where
    O: UrlOpener + Send + Sync + 'static,
    P: ProcessInspector + Send + Sync + 'static,
{
    let settings = Arc::new(GameSettings::new(
        JsonConfigStore::new(data.config_file()),
        GameService::new(steam, FsGameFiles),
    ));
    let session_lock = Arc::new(AtomicSessionLock::default());
    let profiles = Arc::new(
        ProfileService::new(JsonProfileStore::new(data.profiles_file()), SystemClock)
            .with_session(session_lock.clone()),
    );
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
        session_lock,
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
        artwork: Arc::new(ArtworkService::new(artwork)),
        diagnostics: Arc::new(DiagnosticsService::new(
            Arc::new(FileLog::new(data.logs_dir())),
            SysinfoOs,
            folder_opener.clone(),
        )),
        storage: Arc::new(StorageService::new(
            data.root().to_path_buf(),
            FsStorageScanner,
            folder_opener,
        )),
        installer: Arc::new(CatalogInstaller::new(
            downloader,
            FsDownloadStaging::new(data.downloads_dir()),
            Sha256Hasher,
        )),
        install_cancels: Arc::default(),
        downloads: Arc::new(DownloadStatsService::new(
            Arc::new(UnavailableDownloadStats),
            Arc::new(FsInstallIdStore::new(data.install_id_file())),
        )),
        catalog: Arc::new(with_bundled(
            CatalogService::new(
                catalog_source,
                JsonCatalogCache::new(data.catalog_cache_file()),
                SystemClock,
            ),
            bundled_catalog,
        )),
        web_opener,
        ad_opener,
        community,
        // Offline until the bootstrap (or a test) swaps in the real service.
        stars: Arc::new(CommunityStarsService::new(
            Arc::new(UnavailableModStatsApi::new("not configured")),
            Arc::new(MemoryCredentialStore::default()),
        )),
        comments: offline_comments(),
        publish: Arc::new(CommunityPublishService::new(
            Arc::new(UnavailablePublishApi::new("not configured")),
            Arc::new(FsPublishFiles),
            Arc::new(MemoryCredentialStore::default()),
        )),
        publish_cancel: Arc::default(),
        ads: Arc::new(AdService::new(
            ad_source,
            Arc::new(JsonAdCache::new(data.ad_cache_file())) as Arc<dyn AdCache>,
            ad_beacon,
            SystemClock,
        )),
    }
}

impl AppState {
    /// Replaces the stars service (bootstrap and tests).
    pub fn with_stars(self, stars: Arc<CommunityStarsService>) -> Self {
        Self { stars, ..self }
    }

    /// Replaces the publish service (bootstrap and tests).
    pub fn with_publish(self, publish: Arc<CommunityPublishService>) -> Self {
        Self { publish, ..self }
    }

    /// Replaces the community service (tests).
    pub fn with_community(self, community: Arc<CommunityAuthService>) -> Self {
        Self { community, ..self }
    }

    /// Replaces the comments service (production wiring and tests).
    pub fn with_comments(self, comments: Arc<CommunityCommentsService>) -> Self {
        Self { comments, ..self }
    }

    /// Replaces the download-stats service (production wiring and tests).
    pub fn with_downloads(self, downloads: Arc<DownloadStatsService>) -> Self {
        Self { downloads, ..self }
    }

    /// Replaces the ad service (tests).
    pub fn with_ads(self, ads: Arc<Ads>) -> Self {
        Self { ads, ..self }
    }
}
