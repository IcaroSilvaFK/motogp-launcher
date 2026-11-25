//! Shared harness for the interface tests: a Tauri mock-runtime app wired
//! with the real application services over a temp data dir, and fake
//! opener / process / background / event ports. No real window, Steam or game.
#![allow(dead_code)]
use motogp_application::{
    AppError, BackgroundRunner, FolderOpener, ProcessInspector, SessionEvent, SessionPublisher,
    UrlOpener,
};
use motogp_infrastructure::{DataDir, SteamArtwork, SteamLibraries};
use motogp_launcher_lib::{compose, configure, run_recovery, AppState};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};
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
    pub runner: Arc<ManualRunner>,
    pub recorder: Arc<Recorder>,
    pub app: App<MockRuntime>,
    webview: WebviewWindow<MockRuntime>,
}

impl Harness {
    pub fn new() -> Harness {
        let tmp = TempDir::new().unwrap();
        let data = DataDir::new(tmp.path().join("data"));
        fs::create_dir_all(data.root()).unwrap();
        let opener = FakeOpener::default();
        let opened = opener.opened.clone();
        let folder_opener = FakeFolderOpener::default();
        let folders_opened = folder_opener.opened.clone();
        let runner = Arc::new(ManualRunner::default());
        let recorder = Arc::new(Recorder::default());
        let state = compose(
            &data,
            SteamLibraries::new(None),
            SteamArtwork::new(Some(tmp.path().join("steam"))),
            opener,
            Arc::new(folder_opener),
            NoProcesses,
            recorder.clone(),
            runner.clone(),
        );
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
            runner,
            recorder,
            app,
            webview,
        }
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
