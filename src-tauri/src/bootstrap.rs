//! Tauri app bootstrap and composition root. Excluded from coverage (see
//! `scripts/check-rust-coverage.mjs` and `docs/validation-workflow.md`): it
//! only runs with a real window and WebView2; everything it wires is tested
//! through `compose`, `commands`, `adapters` and `startup`.
use crate::adapters::{TauriEvents, TauriFolderOpener, TauriOpener, TauriWebOpener};
use crate::ads_fallback::{NullAdBeacon, NullAdSource};
use crate::{compose, configure, run_recovery, UnavailableCatalogSource};
use motogp_application::{AdBeaconSink, AdSource, CatalogSource, InstanceGuard, InstanceLock};
use motogp_infrastructure::{
    bundled_catalog, DataDir, FileInstanceLock, HttpAdBeaconSink, HttpAdConfig, HttpAdSource,
    HttpCatalogConfig, HttpCatalogSource, SteamArtwork, SteamLibraries, SysinfoProcesses,
    SystemClock, ThreadRunner,
};
use std::sync::{Arc, Mutex};
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
    match HttpCatalogSource::new(HttpCatalogConfig::github(env!("CARGO_PKG_VERSION"))) {
        Ok(source) => Arc::new(source),
        Err(error) => {
            eprintln!("catalog source unavailable: {error}");
            Arc::new(UnavailableCatalogSource::new(error.to_string()))
        }
    }
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
            bundled_catalog()
                .map_err(|error| eprintln!("bundled catalog unavailable: {error}"))
                .ok(),
        );
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
