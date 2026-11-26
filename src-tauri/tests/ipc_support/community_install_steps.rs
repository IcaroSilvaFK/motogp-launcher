//! Steps for `tests/features/community-install.feature` (issue #130): the
//! community server's `GET /catalog` is served by a local HTTP server and read
//! by the real `HttpCommunityCatalogSource`, merged with a GitHub catalog by
//! the real `MergedCatalogSource`, and installed through the real
//! `install_catalog_entry` command. Only the file download is faked: the
//! catalog domain accepts https URLs only, so a loopback http file URL cannot
//! be listed; the fake serves the bytes and records the requested URL.
use crate::ipc_support::{FakeCatalogSource, Harness};
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    CancelFlag, ContentHasher, DownloadError, DownloadProgress, DownloadRequest,
    MergedCatalogSource, ModDownloader,
};
use motogp_infrastructure::{HttpCommunityCatalogSource, HttpCommunityConfig, Sha256Hasher};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

const CONTENT: &[u8] = b"brazil-livery-pak-content";
const FILE_URL: &str = "https://community.example/files/brazil-livery/1.0/Brazil_Livery_P.pak";

/// Serves the bytes in place of the network and records the requested URLs.
struct RecordingDownloader {
    requested: Mutex<Vec<String>>,
}

impl ModDownloader for RecordingDownloader {
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        _cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        self.requested.lock().unwrap().push(request.url.clone());
        std::fs::write(dest, CONTENT).map_err(|e| DownloadError::Storage(e.to_string()))?;
        progress(DownloadProgress {
            received: CONTENT.len() as u64,
            total: request.max_bytes,
        });
        Ok(())
    }
}

#[derive(Default, World)]
struct CommunityInstallWorld {
    harness: Option<Harness>,
    downloader: Option<Arc<RecordingDownloader>>,
    seen_in_discover: bool,
    result: Option<Result<Value, Value>>,
}

impl std::fmt::Debug for CommunityInstallWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommunityInstallWorld")
    }
}

fn sha256_of(content: &[u8]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("x");
    std::fs::write(&file, content).unwrap();
    Sha256Hasher.sha256_hex(&file).unwrap()
}

/// Local stand-in for the community server: answers every request with the
/// given `GET /catalog` body. Returns its `http://127.0.0.1:<port>` base.
fn serve_catalog(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut raw = Vec::new();
            let mut chunk = [0u8; 4096];
            while !String::from_utf8_lossy(&raw).contains("\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => raw.extend_from_slice(&chunk[..n]),
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    base
}

fn community_with(w: &mut CommunityInstallWorld, sha256: String) {
    let body = json!({
        "schemaVersion": 1,
        "entries": [{
            "id": "brazil-livery",
            "title": "Brazil Livery",
            "author": "Ana",
            "game": "MotoGP 26",
            "category": "liveries",
            "description": "A green and yellow livery.",
            "version": "1.0",
            "updatedAt": "2026-11-26",
            "pageUrl": "https://community.example/mods/brazil-livery",
            "download": { "url": FILE_URL, "size": CONTENT.len(), "sha256": sha256 }
        }]
    })
    .to_string();
    let base = serve_catalog(body);
    let community =
        HttpCommunityCatalogSource::new(HttpCommunityConfig::new(&base, "0.0.0")).unwrap();
    let merged = MergedCatalogSource::new(FakeCatalogSource::serving(vec![]), community);
    let downloader = Arc::new(RecordingDownloader {
        requested: Mutex::new(Vec::new()),
    });
    let h = Harness::with_services(Arc::new(merged), downloader.clone());
    h.recover().expect("recovery");
    w.harness = Some(h);
    w.downloader = Some(downloader);
}

#[given(expr = "an admin approved {string} 1.0")]
fn approved(w: &mut CommunityInstallWorld, _title: String) {
    community_with(w, sha256_of(CONTENT));
}

#[given(expr = "the community server lists {string} 1.0 with a wrong SHA-256")]
fn wrong_hash(w: &mut CommunityInstallWorld, _title: String) {
    community_with(w, "0".repeat(64));
}

#[when("a player opens it in Discover and chooses Install")]
fn opens_and_installs(w: &mut CommunityInstallWorld) {
    let h = w.harness.as_ref().expect("harness");
    let catalog = h.invoke("get_catalog", json!({})).unwrap();
    let entry = catalog["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "brazil-livery");
    w.seen_in_discover = entry.is_some_and(|e| e["title"] == "Brazil Livery");
    w.result = Some(h.invoke(
        "install_catalog_entry",
        json!({ "id": "brazil-livery", "enableAfterInstall": false }),
    ));
}

#[then("the file downloads from the community server, its SHA-256 is checked and it is installed")]
fn installed(w: &mut CommunityInstallWorld) {
    assert!(w.seen_in_discover, "the community entry is in the catalog");
    let outcome = w.result.clone().expect("install ran").expect("install ok");
    assert_eq!(outcome["mods"].as_array().unwrap().len(), 1);
    let h = w.harness.as_ref().unwrap();
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), outcome["mods"]);
    assert_eq!(
        *w.downloader.as_ref().unwrap().requested.lock().unwrap(),
        vec![FILE_URL.to_string()]
    );
}

#[then(expr = "the install fails with code {string} and nothing is installed")]
fn fails(w: &mut CommunityInstallWorld, code: String) {
    assert!(w.seen_in_discover, "the community entry is in the catalog");
    let error = w.result.clone().expect("install ran").unwrap_err();
    assert_eq!(error["code"], code.as_str());
    let h = w.harness.as_ref().unwrap();
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), json!([]));
}

/// Runs only `community-install.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CommunityInstallWorld::cucumber()
        .fail_on_skipped()
        .run(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/features/community-install.feature"))
        .await;
    !writer.execution_has_failed()
}
