//! Shared harness for the interface tests: a Tauri mock-runtime app wired
//! with the real application services over a temp data dir, and fake
//! opener / process / background / event ports. No real window, Steam or game.
#![allow(dead_code)]
use motogp_application::{
    AdBeaconError, AdBeaconSink, AdCache, AdCacheError, AdPayload, AdService, AdSource,
    AdSourceError, AppError, AvatarFetcher, BackgroundRunner, CachedAds, CatalogSource,
    CatalogSourceError, CommunityApi, CommunityAuthService, CredentialStore, FolderOpener,
    LoopbackError, LoopbackReceiver, LoopbackSession, ModDownloader, PkceSource, ProcessInspector,
    RawAd, RawCatalog, RawEntry, SessionEvent, SessionPublisher, SourceFetch, UrlOpener,
    WebPageOpener,
};
use motogp_infrastructure::{DataDir, SteamArtwork, SteamLibraries, SystemClock};
use motogp_launcher_lib::{
    compose_with_downloader, configure, production_downloader, run_recovery, Ads, AppState,
    MemoryCredentialStore, UnavailableCommunityApi,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewWindow, WebviewWindowBuilder};
use tempfile::TempDir;

#[derive(Clone, Default)]
pub struct FakeOpener {
    pub opened: Arc<Mutex<Vec<String>>>,
    pub folders_opened: Arc<Mutex<Vec<PathBuf>>>,
}

impl UrlOpener for FakeOpener {
    fn open(&self, url: &str) -> Result<(), AppError> {
        self.opened.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

/// Records the folders it was asked to open instead of starting a file manager.
#[derive(Clone, Default)]
pub struct FakeFolderOpener {
    pub opened: Arc<Mutex<Vec<PathBuf>>>,
}

impl FolderOpener for FakeFolderOpener {
    fn open_folder(&self, path: &Path) -> Result<(), AppError> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// Records the web pages it was asked to open instead of starting a browser.
#[derive(Clone, Default)]
pub struct FakeWebOpener {
    pub opened: Arc<Mutex<Vec<String>>>,
}

impl WebPageOpener for FakeWebOpener {
    fn open_page(&self, url: &str) -> Result<(), AppError> {
        self.opened.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

/// Feed that serves the given creatives (one fetch result per call).
pub struct FakeAdSource {
    creatives: Vec<motogp_domain::AdCreativeDraft>,
}

impl FakeAdSource {
    pub fn serving(creatives: Vec<motogp_domain::AdCreativeDraft>) -> Self {
        Self { creatives }
    }
}

impl AdSource for FakeAdSource {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        Ok(AdPayload {
            creatives: self.creatives.iter().cloned().map(RawAd::Draft).collect(),
        })
    }
}

struct NoAdCache;

impl AdCache for NoAdCache {
    fn load(&self) -> Result<Option<CachedAds>, AdCacheError> {
        Ok(None)
    }
    fn save(&self, _ads: &CachedAds) -> Result<(), AdCacheError> {
        Ok(())
    }
}

struct NoAdBeacon;

impl AdBeaconSink for NoAdBeacon {
    fn send_impression(&self, _url: &str) -> Result<(), AdBeaconError> {
        Ok(())
    }
}

/// An ad service over `source`, with no cache and no beacon.
pub fn ads_over(source: Arc<dyn AdSource>) -> Arc<Ads> {
    Arc::new(AdService::new(
        source,
        Arc::new(NoAdCache) as Arc<dyn AdCache>,
        Arc::new(NoAdBeacon) as Arc<dyn AdBeaconSink>,
        SystemClock,
    ))
}

/// Catalog source that never touches the network: serves the given drafts, or
/// fails as unreachable.
pub struct FakeCatalogSource {
    entries: Option<Vec<motogp_domain::CatalogEntryDraft>>,
    error: CatalogSourceError,
}

impl FakeCatalogSource {
    pub fn failing() -> Self {
        Self {
            entries: None,
            error: CatalogSourceError::Network("offline".to_string()),
        }
    }

    /// Fails with the given error instead of an offline network.
    pub fn failing_with(error: CatalogSourceError) -> Self {
        Self {
            entries: None,
            error,
        }
    }

    pub fn serving(entries: Vec<motogp_domain::CatalogEntryDraft>) -> Self {
        Self {
            entries: Some(entries),
            error: CatalogSourceError::Network("offline".to_string()),
        }
    }
}

impl CatalogSource for FakeCatalogSource {
    fn fetch(&self, _etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        match &self.entries {
            Some(entries) => Ok(SourceFetch::Fetched {
                catalog: RawCatalog {
                    schema_version: 1,
                    entries: entries.iter().cloned().map(RawEntry::Draft).collect(),
                },
                etag: None,
            }),
            None => Err(self.error.clone()),
        }
    }
}

pub struct NoProcesses;

impl ProcessInspector for NoProcesses {
    fn game_running(&self, _root: &Path) -> Result<bool, AppError> {
        Ok(false)
    }
    fn terminate_inside(&self, _root: &Path) -> Result<usize, AppError> {
        Ok(0)
    }
}

type Task = Box<dyn FnOnce() + Send + 'static>;

#[derive(Default)]
pub struct ManualRunner(pub Mutex<Vec<Task>>);

impl BackgroundRunner for ManualRunner {
    fn spawn(&self, task: Task) -> Result<(), motogp_application::SpawnError> {
        self.0.lock().unwrap().push(task);
        Ok(())
    }
}

#[derive(Default)]
pub struct Recorder {
    pub events: Mutex<Vec<SessionEvent>>,
    pub errors: Mutex<Vec<String>>,
}

impl SessionPublisher for Recorder {
    fn publish(&self, event: SessionEvent) {
        self.events.lock().unwrap().push(event);
    }
    fn report_error(&self, message: &str) {
        self.errors.lock().unwrap().push(message.to_string());
    }
}

/// Sends a command through the IPC layer, like the WebView would.
pub fn invoke_on(
    webview: &WebviewWindow<MockRuntime>,
    cmd: &str,
    body: Value,
) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "http://tauri.localhost".parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(webview, request)
        .map(|body| body.deserialize::<Value>().expect("JSON response"))
}

pub struct Harness {
    pub tmp: TempDir,
    pub opened: Arc<Mutex<Vec<String>>>,
    pub folders_opened: Arc<Mutex<Vec<PathBuf>>>,
    pub pages_opened: Arc<Mutex<Vec<String>>>,
    /// Sponsor pages opened through the ad opener.
    pub ad_pages_opened: Arc<Mutex<Vec<String>>>,
    pub runner: Arc<ManualRunner>,
    pub recorder: Arc<Recorder>,
    pub app: App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
}

impl Harness {
    pub fn new() -> Harness {
        Harness::with_catalog_source(Arc::new(FakeCatalogSource::failing()))
    }

    pub fn with_catalog_source(catalog_source: Arc<dyn CatalogSource>) -> Harness {
        Harness::with_services(catalog_source, production_downloader())
    }

    /// Catalog source and mod downloader injected (#74): no network either way.
    pub fn with_services(
        catalog_source: Arc<dyn CatalogSource>,
        downloader: Arc<dyn ModDownloader>,
    ) -> Harness {
        Harness::with_bundled(catalog_source, downloader, None)
    }

    /// Like [`Harness::with_services`], with a catalog snapshot bundled in the build (#91).
    pub fn with_bundled(
        catalog_source: Arc<dyn CatalogSource>,
        downloader: Arc<dyn ModDownloader>,
        bundled_catalog: Option<RawCatalog>,
    ) -> Harness {
        Harness::build(
            catalog_source,
            downloader,
            bundled_catalog,
            None,
            None,
            None,
        )
    }

    /// The composed state with this feed behind the real ad service and its
    /// disk cache (#103).
    pub fn with_ad_source(source: Arc<dyn AdSource>) -> Harness {
        Harness::build(
            Arc::new(FakeCatalogSource::failing()),
            production_downloader(),
            None,
            None,
            Some(source),
            None,
        )
    }

    /// The default composed state with the community service replaced (#122).
    pub fn with_community(community: Arc<CommunityAuthService>) -> Harness {
        Harness::build(
            Arc::new(FakeCatalogSource::failing()),
            production_downloader(),
            None,
            None,
            None,
            Some(community),
        )
    }

    /// The default composed state with the ad service replaced (#103).
    pub fn with_ads(ads: Arc<Ads>) -> Harness {
        Harness::build(
            Arc::new(FakeCatalogSource::failing()),
            production_downloader(),
            None,
            Some(ads),
            None,
            None,
        )
    }

    fn build(
        catalog_source: Arc<dyn CatalogSource>,
        downloader: Arc<dyn ModDownloader>,
        bundled_catalog: Option<RawCatalog>,
        ads: Option<Arc<Ads>>,
        ad_source: Option<Arc<dyn AdSource>>,
        community: Option<Arc<CommunityAuthService>>,
    ) -> Harness {
        let tmp = TempDir::new().unwrap();
        let data = DataDir::new(tmp.path().join("data"));
        fs::create_dir_all(data.root()).unwrap();
        let opener = FakeOpener::default();
        let opened = opener.opened.clone();
        let folder_opener = FakeFolderOpener::default();
        let folders_opened = folder_opener.opened.clone();
        let web_opener = FakeWebOpener::default();
        let pages_opened = web_opener.opened.clone();
        let ad_opener = FakeWebOpener::default();
        let ad_pages_opened = ad_opener.opened.clone();
        let runner = Arc::new(ManualRunner::default());
        let recorder = Arc::new(Recorder::default());
        let state = compose_with_downloader(
            &data,
            SteamLibraries::new(None),
            SteamArtwork::new(Some(tmp.path().join("steam"))),
            opener,
            Arc::new(folder_opener),
            Arc::new(web_opener),
            Arc::new(ad_opener),
            NoProcesses,
            recorder.clone(),
            runner.clone(),
            catalog_source,
            ad_source.unwrap_or_else(|| Arc::new(FakeAdSource::serving(vec![]))),
            Arc::new(NoAdBeacon),
            community_over(
                Arc::new(UnavailableCommunityApi::new("offline in tests")),
                Arc::new(MemoryCredentialStore::default()),
                Arc::new(NoLoopback),
            ),
            bundled_catalog,
            downloader,
        );
        let state = match community {
            Some(community) => state.with_community(community),
            None => state,
        };
        let state = match ads {
            Some(ads) => state.with_ads(ads),
            None => state,
        };
        let app = configure(tauri::test::mock_builder())
            .manage(state)
            .build(mock_context(noop_assets()))
            .expect("mock app");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview");
        Harness {
            tmp,
            opened,
            folders_opened,
            pages_opened,
            ad_pages_opened,
            runner,
            recorder,
            app,
            webview,
        }
    }

    /// Records the payloads of an app event, parsed as JSON, from now on.
    pub fn capture_events(&self, event: &str) -> Arc<Mutex<Vec<Value>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        self.app.listen(event, move |e| {
            let payload: Value = serde_json::from_str(e.payload()).expect("JSON payload");
            sink.lock().unwrap().push(payload);
        });
        seen
    }

    pub fn data_dir(&self) -> PathBuf {
        self.tmp.path().join("data")
    }

    /// Runs the startup recovery exactly like the bootstrap does.
    pub fn recover(&self) -> Result<usize, Value> {
        run_recovery(&self.app.state::<AppState>()).map_err(|e| json!(e))
    }

    /// Sends a command through the IPC layer, like the WebView would.
    pub fn invoke(&self, cmd: &str, body: Value) -> Result<Value, Value> {
        invoke_on(&self.webview, cmd, body)
    }

    /// A handle other threads can use to invoke commands concurrently.
    pub fn webview(&self) -> WebviewWindow<MockRuntime> {
        self.webview.clone()
    }

    /// A valid Unreal install `steam/steamapps/common/MotoGP25` with an App ID.
    pub fn create_game(&self) -> PathBuf {
        let steamapps = self.tmp.path().join("steam").join("steamapps");
        let root = steamapps.join("common").join("MotoGP25");
        fs::create_dir_all(root.join("MotoGP25").join("Content").join("Paks")).unwrap();
        fs::write(
            steamapps.join("appmanifest_123.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"123\"\n\t\"installdir\"\t\t\"MotoGP25\"\n}\n",
        )
        .unwrap();
        root
    }

    pub fn mods_dir(&self, root: &Path) -> PathBuf {
        root.join("MotoGP25")
            .join("Content")
            .join("Paks")
            .join("~mods")
    }

    pub fn write_mod_source(&self, name: &str, content: &[u8]) -> PathBuf {
        let dir = self.tmp.path().join("src");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }
}

/// Fixed `state` and verifier so tests can predict what the browser gets back.
pub struct FixedPkce;

impl PkceSource for FixedPkce {
    fn state(&self) -> String {
        "state-1".to_string()
    }
    fn verifier(&self) -> String {
        "verifier-1".to_string()
    }
    fn challenge(&self, verifier: &str) -> String {
        format!("challenge-of-{verifier}")
    }
}

/// Avatars are never fetched in the interface tests.
pub struct NoAvatarFetcher;

impl AvatarFetcher for NoAvatarFetcher {
    fn fetch(&self, _url: &str) -> Option<String> {
        None
    }
}

/// A loopback that cannot bind: sign-in fails before opening a browser.
pub struct NoLoopback;

impl LoopbackReceiver for NoLoopback {
    fn bind(&self) -> Result<Box<dyn LoopbackSession>, LoopbackError> {
        Err(LoopbackError::Io("no loopback in tests".to_string()))
    }
}

/// The community service over fake ports (never a network, a browser or the
/// Credential Manager).
pub fn community_over(
    api: Arc<dyn CommunityApi>,
    store: Arc<dyn CredentialStore>,
    loopback: Arc<dyn LoopbackReceiver>,
) -> Arc<CommunityAuthService> {
    Arc::new(CommunityAuthService::new(
        api,
        store,
        loopback,
        Arc::new(FixedPkce),
        Arc::new(FakeWebOpener::default()),
        Arc::new(NoAvatarFetcher),
    ))
}
