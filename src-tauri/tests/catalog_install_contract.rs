//! IPC contract of the catalog install commands (#74), through the Tauri mock
//! runtime with a fake catalog source and a fake downloader (never the
//! network). Test names follow the Gherkin scenarios of the issue.
mod ipc_support;

use ipc_support::{FakeCatalogSource, Harness};
use motogp_application::{
    CancelFlag, ContentHasher, DownloadError, DownloadProgress, DownloadRequest, ModDownloader,
};
use motogp_domain::{CatalogDownloadDraft, CatalogEntryDraft};
use motogp_infrastructure::Sha256Hasher;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CONTENT: &[u8] = b"rebalanced-pak-content";
const URL: &str = "https://example.com/files/Rebalanced_P.pak";

#[derive(Clone)]
enum Behavior {
    Serve,
    Fail(DownloadError),
    /// Blocks until the cancel flag is raised, then reports `Cancelled`.
    WaitForCancel,
}

struct FakeDownloader {
    content: Vec<u8>,
    behavior: Behavior,
    requested: Mutex<Vec<String>>,
    started: AtomicBool,
}

impl FakeDownloader {
    fn new(behavior: Behavior) -> Arc<Self> {
        Arc::new(Self {
            content: CONTENT.to_vec(),
            behavior,
            requested: Mutex::new(Vec::new()),
            started: AtomicBool::new(false),
        })
    }

    fn requested(&self) -> Vec<String> {
        self.requested.lock().unwrap().clone()
    }

    fn wait_started(&self) {
        let until = Instant::now() + Duration::from_secs(10);
        while !self.started.load(Ordering::SeqCst) {
            assert!(Instant::now() < until, "the download never started");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl ModDownloader for FakeDownloader {
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        self.requested.lock().unwrap().push(request.url.clone());
        self.started.store(true, Ordering::SeqCst);
        match &self.behavior {
            Behavior::Fail(error) => Err(error.clone()),
            Behavior::WaitForCancel => {
                let until = Instant::now() + Duration::from_secs(10);
                while !cancel.is_cancelled() && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(DownloadError::Cancelled)
            }
            Behavior::Serve => {
                fs::write(dest, &self.content)
                    .map_err(|e| DownloadError::Storage(e.to_string()))?;
                progress(DownloadProgress {
                    received: self.content.len() as u64,
                    total: request.max_bytes,
                });
                Ok(())
            }
        }
    }
}

fn sha256_of(content: &[u8]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("x");
    fs::write(&file, content).unwrap();
    Sha256Hasher.sha256_hex(&file).unwrap()
}

fn draft(id: &str, download: Option<(u64, String)>) -> CatalogEntryDraft {
    CatalogEntryDraft {
        id: id.to_string(),
        title: format!("Title of {id}"),
        author: "Someone".to_string(),
        game: "MotoGP 26".to_string(),
        category: "AI".to_string(),
        description: "Fairer riders.".to_string(),
        version: "1.2".to_string(),
        updated_at: "2026-11-01".to_string(),
        page_url: format!("https://example.com/{id}"),
        download: download.map(|(size, sha256)| CatalogDownloadDraft {
            url: URL.to_string(),
            size: Some(size),
            sha256: Some(sha256),
        }),
        ..Default::default()
    }
}

fn matching() -> CatalogEntryDraft {
    draft(
        "rebalanced-ai",
        Some((CONTENT.len() as u64, sha256_of(CONTENT))),
    )
}

fn harness(entries: Vec<CatalogEntryDraft>, downloader: &Arc<FakeDownloader>) -> Harness {
    Harness::with_services(
        Arc::new(FakeCatalogSource::serving(entries)),
        downloader.clone(),
    )
}

fn install(h: &Harness) -> Result<Value, Value> {
    h.invoke("install_catalog_entry", json!({ "id": "rebalanced-ai" }))
}

fn downloads_dir_is_clean(h: &Harness) -> bool {
    let dir = h.data_dir().join("downloads");
    !dir.exists() || fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn a_downloadable_entry_is_installed_into_the_library() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = harness(vec![matching()], &downloader);
    let events = h.capture_events("catalog-install-progress");

    let installed = install(&h).unwrap();

    assert_eq!(installed.as_array().unwrap().len(), 1);
    assert_eq!(installed[0]["name"], "Rebalanced");
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), installed);
    assert!(downloads_dir_is_clean(&h));
    assert_eq!(downloader.requested(), vec![URL.to_string()]);

    // Progress events: camelCase payload, phases in order.
    let seen = events.lock().unwrap().clone();
    let phases: Vec<&str> = seen.iter().map(|e| e["phase"].as_str().unwrap()).collect();
    assert_eq!(phases, ["downloading", "verifying", "importing"]);
    let first = seen[0].as_object().unwrap();
    let mut keys: Vec<&str> = first.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["id", "phase", "received", "total"]);
    assert_eq!(seen[0]["id"], "rebalanced-ai");
    assert_eq!(seen[0]["received"], CONTENT.len());
    assert_eq!(seen[0]["total"], CONTENT.len());
}

#[test]
fn the_command_takes_only_an_id_and_never_a_url() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = harness(vec![matching()], &downloader);

    let missing = h.invoke("install_catalog_entry", json!({}));
    assert!(missing.is_err(), "the id is required");

    install_with_extra_url(&h);
    assert_eq!(
        downloader.requested(),
        vec![URL.to_string()],
        "the URL always comes from the catalog"
    );
}

fn install_with_extra_url(h: &Harness) {
    h.invoke(
        "install_catalog_entry",
        json!({ "id": "rebalanced-ai", "url": "https://evil.example/x.pak" }),
    )
    .unwrap();
}

#[test]
fn an_unknown_entry_maps_to_catalog_entry_not_found() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = harness(vec![matching()], &downloader);

    let error = h
        .invoke("install_catalog_entry", json!({ "id": "ghost" }))
        .unwrap_err();

    assert_eq!(error["code"], "catalog_entry_not_found");
    assert!(downloader.requested().is_empty());
}

#[test]
fn an_entry_without_download_maps_to_not_downloadable() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = harness(vec![draft("rebalanced-ai", None)], &downloader);

    let error = install(&h).unwrap_err();

    assert_eq!(error["code"], "catalog_entry_not_downloadable");
    assert!(downloader.requested().is_empty());
}

#[test]
fn a_size_mismatch_maps_to_download_size_mismatch() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let wrong = draft("rebalanced-ai", Some((999, sha256_of(CONTENT))));
    let h = harness(vec![wrong], &downloader);

    let error = install(&h).unwrap_err();

    assert_eq!(error["code"], "download_size_mismatch");
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), json!([]));
    assert!(downloads_dir_is_clean(&h));
}

#[test]
fn a_hash_mismatch_maps_to_download_hash_mismatch() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let wrong = draft(
        "rebalanced-ai",
        Some((CONTENT.len() as u64, "0".repeat(64))),
    );
    let h = harness(vec![wrong], &downloader);

    let error = install(&h).unwrap_err();

    assert_eq!(error["code"], "download_hash_mismatch");
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), json!([]));
    assert!(downloads_dir_is_clean(&h));
}

#[test]
fn a_network_failure_maps_to_download_network_without_leaking_details() {
    let downloader = FakeDownloader::new(Behavior::Fail(DownloadError::Network(
        "dns error for secret.host".into(),
    )));
    let h = harness(vec![matching()], &downloader);

    let error = install(&h).unwrap_err();

    assert_eq!(error["code"], "download_network");
    assert!(!error["message"].as_str().unwrap().contains("secret"));
    assert!(downloads_dir_is_clean(&h));
}

#[test]
fn a_storage_failure_maps_to_write_failed() {
    let downloader =
        FakeDownloader::new(Behavior::Fail(DownloadError::Storage("disk full".into())));
    let h = harness(vec![matching()], &downloader);

    assert_eq!(install(&h).unwrap_err()["code"], "write_failed");
}

#[test]
fn a_downloader_that_reports_cancelled_maps_to_download_cancelled() {
    let downloader = FakeDownloader::new(Behavior::Fail(DownloadError::Cancelled));
    let h = harness(vec![matching()], &downloader);

    assert_eq!(install(&h).unwrap_err()["code"], "download_cancelled");
}

#[test]
fn an_unavailable_catalog_reuses_the_catalog_code() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = Harness::with_services(Arc::new(FakeCatalogSource::failing()), downloader.clone());

    assert_eq!(install(&h).unwrap_err()["code"], "catalog_unavailable");
}

#[test]
fn a_file_the_library_cannot_import_reuses_the_library_code() {
    // A staged ".txt" file passes the checks but is not an importable format.
    let downloader = FakeDownloader::new(Behavior::Serve);
    let mut entry = matching();
    entry.download.as_mut().unwrap().url = "https://example.com/files/readme.txt".into();
    let h = harness(vec![entry], &downloader);

    assert_eq!(install(&h).unwrap_err()["code"], "unsupported_format");
    assert!(downloads_dir_is_clean(&h));
}

#[test]
fn installs_are_refused_while_a_session_is_running() {
    let downloader = FakeDownloader::new(Behavior::Serve);
    let h = harness(vec![matching()], &downloader);
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    h.recover().unwrap();
    h.invoke("play", json!({ "profileId": "vanilla" })).unwrap();

    let error = install(&h).unwrap_err();

    assert_eq!(error["code"], "library_locked_by_session");
    assert!(downloader.requested().is_empty(), "nothing was downloaded");
    assert!(downloads_dir_is_clean(&h));
}

#[test]
fn cancelling_stops_the_install_and_a_second_request_is_refused_meanwhile() {
    let downloader = FakeDownloader::new(Behavior::WaitForCancel);
    let h = harness(vec![matching()], &downloader);
    let webview = h.webview();
    let first = std::thread::spawn(move || {
        ipc_support::invoke_on(
            &webview,
            "install_catalog_entry",
            json!({ "id": "rebalanced-ai" }),
        )
    });
    downloader.wait_started();

    let second = install(&h).unwrap_err();
    assert_eq!(second["code"], "catalog_install_in_progress");

    assert_eq!(
        h.invoke("cancel_catalog_install", json!({ "id": "rebalanced-ai" }))
            .unwrap(),
        Value::Null
    );
    let first = first.join().unwrap().unwrap_err();
    assert_eq!(first["code"], "download_cancelled");
    assert!(downloads_dir_is_clean(&h));

    // The slot is free again once the install ended.
    assert_eq!(
        h.invoke("cancel_catalog_install", json!({ "id": "rebalanced-ai" }))
            .unwrap(),
        Value::Null,
        "cancelling with nothing in flight is a no-op"
    );
}
