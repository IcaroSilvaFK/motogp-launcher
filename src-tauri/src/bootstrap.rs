//! Tauri app bootstrap and composition root. Excluded from coverage (see
//! `scripts/check-rust-coverage.mjs` and `docs/validation-workflow.md`): it
//! only runs with a real window and WebView2; everything it wires is tested
//! through `compose`, `commands`, `adapters` and `startup`.
use crate::adapters::{TauriEvents, TauriFolderOpener, TauriOpener, TauriWebOpener};
use crate::ads_fallback::{NullAdBeacon, NullAdSource};
use crate::{
    compose, configure, run_recovery, MemoryCredentialStore, UnavailableCatalogSource,
    UnavailableCommunityApi, UnavailableDownloadStats, UnavailableModStatsApi,
    UnavailablePublishApi, COMMUNITY_SERVER_URL,
};
use motogp_application::{
    AdBeaconSink, AdSource, AvatarFetcher, CatalogSource, CommunityApi, CommunityAuthService,
    CommunityCommentsApi, CommunityCommentsService, CommunityPublishService, CommunityStarsService,
    CredentialStore, DownloadStatsApi, DownloadStatsService, InstanceGuard, InstanceLock,
    MergedCatalogSource, ModPublishApi, ModStatsApi,
};
use motogp_infrastructure::{
    bundled_catalog, DataDir, FileInstanceLock, FsInstallIdStore, FsPublishFiles, HttpAdBeaconSink,
    HttpAdConfig, HttpAdSource, HttpAvatarFetcher, HttpCatalogConfig, HttpCatalogSource,
    HttpCommunityApi, HttpCommunityCatalogSource, HttpCommunityConfig, HttpDownloadStats,
    HttpModPublishApi, HttpModStatsApi, OsPkce, SteamArtwork, SteamLibraries, SysinfoProcesses,
    SystemClock, TcpLoopbackReceiver, ThreadRunner, WindowsCredentialStore,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

#[cfg(windows)]
fn steam_libraries() -> SteamLibraries {
    SteamLibraries::from_registry()
}

#[cfg(not(windows))]
fn steam_libraries() -> SteamLibraries {
    SteamLibraries::new(None)
}

#[cfg(windows)]
fn steam_artwork() -> SteamArtwork {
    SteamArtwork::new(motogp_infrastructure::read_steam_path())
}

#[cfg(not(windows))]
fn steam_artwork() -> SteamArtwork {
    SteamArtwork::new(None)
}

/// The HTTPS catalog source; if its client cannot be built the catalog is
/// served from the offline cache only and the app still starts.
fn catalog_source() -> Arc<dyn CatalogSource> {
    let version = env!("CARGO_PKG_VERSION");
    let github: Arc<dyn CatalogSource> =
        match HttpCatalogSource::new(HttpCatalogConfig::github(version)) {
            Ok(source) => Arc::new(source),
            Err(error) => {
                eprintln!("catalog source unavailable: {error}");
                Arc::new(UnavailableCatalogSource::new(error.to_string()))
            }
        };
    // The community server's catalog (#130) is merged in: its entries win, and
    // when it is down the catalog is the GitHub one alone.
    let community: Arc<dyn CatalogSource> = match HttpCommunityCatalogSource::new(
        HttpCommunityConfig::new(COMMUNITY_SERVER_URL, version),
    ) {
        Ok(source) => Arc::new(source),
        Err(error) => {
            eprintln!("community catalog unavailable: {error}");
            Arc::new(UnavailableCatalogSource::new(error.to_string()))
        }
    };
    Arc::new(MergedCatalogSource::new(github, community))
}

/// The HTTPS ad source and beacon, built from one config. If either client
/// cannot be built the app starts with the null adapters: no ad is shown.
fn ad_adapters() -> (Arc<dyn AdSource>, Arc<dyn AdBeaconSink>) {
    let mut config = HttpAdConfig::github(env!("CARGO_PKG_VERSION"));
    // Debug builds read the feed from `dev`, so ads merged there can be seen
    // in `tauri dev` before they are promoted to `main`.
    if cfg!(debug_assertions) {
        config.url = config.url.replace("/main/", "/dev/");
        config.raw_base_url = config.raw_base_url.replace("/main/", "/dev/");
    }
    let source: Arc<dyn AdSource> = match HttpAdSource::new(config.clone(), SystemClock) {
        Ok(source) => Arc::new(source),
        Err(error) => {
            eprintln!("ad source unavailable: {error}");
            Arc::new(NullAdSource)
        }
    };
    let beacon: Arc<dyn AdBeaconSink> = match HttpAdBeaconSink::new(&config) {
        Ok(beacon) => Arc::new(beacon),
        Err(error) => {
            eprintln!("ad beacon unavailable: {error}");
            Arc::new(NullAdBeacon)
        }
    };
    (source, beacon)
}

/// The community account: HTTP client, Windows Credential Manager, loopback
/// listener and avatar fetcher. A piece that cannot be built degrades to an
/// offline community (or an in-memory session) instead of stopping the app.
fn community_services(
    handle: tauri::AppHandle,
) -> (
    Arc<CommunityAuthService>,
    Arc<CommunityStarsService>,
    Arc<CommunityCommentsService>,
    Arc<CommunityPublishService>,
) {
    let version = env!("CARGO_PKG_VERSION");
    let (api, comments_api): (Arc<dyn CommunityApi>, Arc<dyn CommunityCommentsApi>) =
        match HttpCommunityApi::new(HttpCommunityConfig::new(COMMUNITY_SERVER_URL, version)) {
            Ok(api) => {
                let api = Arc::new(api);
                (api.clone(), api)
            }
            Err(error) => {
                eprintln!("community server client unavailable: {error}");
                let api = Arc::new(UnavailableCommunityApi::new(error.to_string()));
                (api.clone(), api)
            }
        };
    let store: Arc<dyn CredentialStore> = match WindowsCredentialStore::new() {
        Ok(store) => Arc::new(store),
        Err(error) => {
            eprintln!("credential store unavailable: {error}");
            Arc::new(MemoryCredentialStore::default())
        }
    };
    let avatars: Arc<dyn AvatarFetcher> = match HttpAvatarFetcher::new(version) {
        Ok(fetcher) => Arc::new(fetcher),
        Err(error) => {
            eprintln!("avatar fetcher unavailable: {error}");
            Arc::new(NoAvatars)
        }
    };
    let stars_api: Arc<dyn ModStatsApi> =
        match HttpModStatsApi::new(HttpCommunityConfig::new(COMMUNITY_SERVER_URL, version)) {
            Ok(api) => Arc::new(api),
            Err(error) => {
                eprintln!("community stars client unavailable: {error}");
                Arc::new(UnavailableModStatsApi::new(error.to_string()))
            }
        };
    let stars = Arc::new(CommunityStarsService::new(stars_api, store.clone()));
    let comments = Arc::new(CommunityCommentsService::new(
        comments_api,
        store.clone(),
        avatars.clone(),
    ));
    // One request is a whole 8 MiB chunk: give it far more than the account calls get.
    let mut publish_config = HttpCommunityConfig::new(COMMUNITY_SERVER_URL, version);
    publish_config.timeout = Duration::from_secs(120);
    let publish_api: Arc<dyn ModPublishApi> = match HttpModPublishApi::new(publish_config) {
        Ok(api) => Arc::new(api),
        Err(error) => {
            eprintln!("community publish client unavailable: {error}");
            Arc::new(UnavailablePublishApi::new(error.to_string()))
        }
    };
    let publish = Arc::new(CommunityPublishService::new(
        publish_api,
        Arc::new(FsPublishFiles),
        store.clone(),
    ));
    let auth = Arc::new(CommunityAuthService::new(
        api,
        store,
        Arc::new(TcpLoopbackReceiver),
        Arc::new(OsPkce),
        Arc::new(TauriWebOpener::community(handle)),
        avatars,
    ));
    (auth, stars, comments, publish)
}

/// Anonymous download counts (#120): HTTP client plus the install id file. If the
/// client cannot be built the counts are simply unavailable.
fn download_stats(data: &DataDir) -> Arc<DownloadStatsService> {
    let config = HttpCommunityConfig::new(COMMUNITY_SERVER_URL, env!("CARGO_PKG_VERSION"));
    let api: Arc<dyn DownloadStatsApi> = match HttpDownloadStats::new(config) {
        Ok(api) => Arc::new(api),
        Err(error) => {
            eprintln!("download stats unavailable: {error}");
            Arc::new(UnavailableDownloadStats)
        }
    };
    Arc::new(DownloadStatsService::new(
        api,
        Arc::new(FsInstallIdStore::new(data.install_id_file())),
    ))
}

/// Avatar fetcher used when the HTTP client cannot be built: initials only.
struct NoAvatars;

impl AvatarFetcher for NoAvatars {
    fn fetch(&self, _url: &str) -> Option<String> {
        None
    }
}

/// Keeps the single-instance lock alive in the managed state.
struct InstanceHold(#[allow(dead_code)] Mutex<Option<InstanceGuard>>);

/// Starts the Tauri application.
pub fn run() {
    configure(
        tauri::Builder::default()
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_opener::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init()),
    )
    .setup(|app| {
        let data = DataDir::new(app.path().app_data_dir()?);
        std::fs::create_dir_all(data.root())?;
        // One launcher process at a time: a second one would share the
        // deployment manifest and the game directory. The lock is taken before
        // the recovery and held for the whole app lifetime.
        match FileInstanceLock::new(data.root().join("instance.lock")).acquire()? {
            Some(guard) => {
                app.manage(InstanceHold(Mutex::new(Some(guard))));
            }
            None => {
                eprintln!("another MotoGP Launcher is already running; exiting");
                std::process::exit(0);
            }
        }
        let handle = app.handle().clone();
        let handle_for_community = handle.clone();
        let (community, stars, comments, publish) = community_services(handle_for_community);
        let (ad_source, ad_beacon) = ad_adapters();
        let state = compose(
            &data,
            steam_libraries(),
            steam_artwork(),
            TauriOpener::new(handle.clone()),
            Arc::new(TauriFolderOpener::new(handle.clone())),
            Arc::new(TauriWebOpener::new(handle.clone())),
            Arc::new(TauriWebOpener::https_only(handle.clone())),
            SysinfoProcesses::default(),
            Arc::new(TauriEvents::new(handle)),
            Arc::new(ThreadRunner),
            catalog_source(),
            ad_source,
            ad_beacon,
            community.clone(),
            bundled_catalog()
                .map_err(|error| eprintln!("bundled catalog unavailable: {error}"))
                .ok(),
        )
        .with_stars(stars)
        .with_comments(comments)
        .with_downloads(download_stats(&data))
        .with_publish(publish);
        // Recovery finishes before the state exists, so no command can run
        // `play` earlier. A failure is already on the error channel and blocks
        // new deploys; it must not stop the app from starting.
        if let Err(error) = run_recovery(&state) {
            eprintln!("startup recovery failed: {}", error.message);
        }
        app.manage(state);
        Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running MotoGP Launcher");
}
