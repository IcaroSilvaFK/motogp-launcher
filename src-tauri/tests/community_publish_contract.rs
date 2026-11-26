//! IPC contract of publishing mods to the community server (#130) through the
//! Tauri mock runtime with a fake server and the real file reader: never the
//! network or the Windows Credential Manager.
mod ipc_support;

use ipc_support::{invoke_on, Harness};
use motogp_application::{
    CommunityPublishService, CreateUpload, CredentialError, CredentialStore, ModPublishApi,
    ModVersion, MyMod, PublishApiError, UploadSession, UploadStatus, VersionStatus,
};
use motogp_domain::SessionToken;
use motogp_infrastructure::FsPublishFiles;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::test::MockRuntime;
use tauri::WebviewWindow;

const TOKEN: &str = "tok-SECRET-123";

type Hook = Box<dyn Fn() + Send + Sync>;

/// An in-memory community server.
#[derive(Default)]
struct FakeServer {
    fail_create: Mutex<Option<PublishApiError>>,
    chunks: Mutex<Vec<(u64, usize)>>,
    aborted: Mutex<bool>,
    seen_tokens: Mutex<Vec<String>>,
    on_chunk: Mutex<Option<Hook>>,
}

fn version(status: VersionStatus, reason: Option<&str>) -> ModVersion {
    ModVersion {
        mod_id: "brazil-livery".into(),
        version: "1.0".into(),
        title: "Brazil Livery".into(),
        description: "A livery".into(),
        long_description: None,
        category: "liveries".into(),
        game: "MotoGP 26".into(),
        file_name: "Brazil Livery.zip".into(),
        size: 10,
        sha256: "h".into(),
        status,
        rejection_reason: reason.map(str::to_string),
        screenshot_count: 0,
        uploaded_at: "2026-11-26T10:00:00Z".into(),
        reviewed_at: None,
    }
}

impl FakeServer {
    fn note(&self, token: &SessionToken) {
        self.seen_tokens
            .lock()
            .unwrap()
            .push(token.expose().to_string());
    }
}

impl ModPublishApi for FakeServer {
    fn create_upload(
        &self,
        token: &SessionToken,
        _upload: &CreateUpload,
    ) -> Result<UploadSession, PublishApiError> {
        self.note(token);
        if let Some(error) = self.fail_create.lock().unwrap().take() {
            return Err(error);
        }
        Ok(UploadSession {
            upload_id: "up1".into(),
            chunk_size: 4,
            received: 0,
        })
    }
    fn put_chunk(
        &self,
        token: &SessionToken,
        _id: &str,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, PublishApiError> {
        self.note(token);
        self.chunks.lock().unwrap().push((offset, bytes.len()));
        if let Some(hook) = self.on_chunk.lock().unwrap().as_ref() {
            hook();
        }
        Ok(offset + bytes.len() as u64)
    }
    fn upload_status(
        &self,
        _token: &SessionToken,
        _id: &str,
    ) -> Result<UploadStatus, PublishApiError> {
        Ok(UploadStatus {
            received: 0,
            size: 10,
        })
    }
    fn add_screenshot(
        &self,
        _token: &SessionToken,
        _id: &str,
        _content_type: &str,
        _bytes: &[u8],
    ) -> Result<(), PublishApiError> {
        Ok(())
    }
    fn complete(&self, token: &SessionToken, _id: &str) -> Result<ModVersion, PublishApiError> {
        self.note(token);
        Ok(version(VersionStatus::Pending, None))
    }
    fn abort(&self, _token: &SessionToken, _id: &str) -> Result<(), PublishApiError> {
        *self.aborted.lock().unwrap() = true;
        Ok(())
    }
    fn my_mods(&self, token: &SessionToken) -> Result<Vec<MyMod>, PublishApiError> {
        self.note(token);
        Ok(vec![MyMod {
            id: "brazil-livery".into(),
            versions: vec![
                version(VersionStatus::Rejected, Some("Not allowed")),
                version(VersionStatus::Approved, None),
            ],
        }])
    }
}

struct Store(Option<SessionToken>);

impl CredentialStore for Store {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
        Ok(self.0.clone())
    }
    fn save(&self, _token: &SessionToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self) -> Result<(), CredentialError> {
        Ok(())
    }
}

fn harness(server: &Arc<FakeServer>, token: Option<&str>) -> Harness {
    let store = Arc::new(Store(token.map(SessionToken::new)));
    let service = CommunityPublishService::new(server.clone(), Arc::new(FsPublishFiles), store)
        .with_retry_delay(Duration::ZERO);
    Harness::with_publish(Arc::new(service))
}

/// A real 10-byte mod file in a folder with accents on purpose (real users have them).
fn mod_file(h: &Harness, name: &str) -> PathBuf {
    let dir = h.tmp.path().join("Usuário Padrão");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, b"0123456789").unwrap();
    path
}

fn request(file: &Path) -> Value {
    json!({"request": {
        "modId": "brazil-livery",
        "title": "Brazil Livery",
        "description": "A livery",
        "category": "liveries",
        "game": "MotoGP 26",
        "version": "1.0",
        "file": file.to_string_lossy(),
        "screenshots": [],
        "ownershipConfirmed": true
    }})
}

fn assert_no_token(value: &Value) {
    assert!(!value.to_string().contains(TOKEN), "{value}");
}

#[test]
fn a_creator_publishes_a_mod_and_it_waits_for_review() {
    let server = Arc::new(FakeServer::default());
    let h = harness(&server, Some(TOKEN));
    let events = h.capture_events("community-publish-progress");
    let file = mod_file(&h, "Brazil Livery.zip");
    let result = h.invoke("community_publish_mod", request(&file)).unwrap();
    assert_eq!(result["modId"], json!("brazil-livery"));
    assert_eq!(result["status"], json!("pending"));
    assert_eq!(result["fileName"], json!("Brazil Livery.zip"));
    assert_no_token(&result);
    assert_eq!(*server.chunks.lock().unwrap(), vec![(0, 4), (4, 4), (8, 2)]);
    assert!(server
        .seen_tokens
        .lock()
        .unwrap()
        .iter()
        .all(|t| t == TOKEN));
    let events = events.lock().unwrap();
    let phases: Vec<&str> = events
        .iter()
        .map(|e| e["phase"].as_str().unwrap())
        .collect();
    assert_eq!(phases.first(), Some(&"hashing"));
    assert!(phases.contains(&"uploading"));
    assert!(phases.contains(&"finishing"));
    assert_eq!(phases.last(), Some(&"done"));
    assert_eq!(events.last().unwrap()["sent"], json!(10));
    assert_eq!(events.last().unwrap()["total"], json!(10));
    events.iter().for_each(assert_no_token);
}

#[test]
fn publishing_without_a_session_is_not_signed_in() {
    let server = Arc::new(FakeServer::default());
    let h = harness(&server, None);
    let file = mod_file(&h, "Brazil Livery.zip");
    let error = h
        .invoke("community_publish_mod", request(&file))
        .unwrap_err();
    assert_eq!(error["code"], json!("not_signed_in"));
    assert!(server.chunks.lock().unwrap().is_empty());
}

#[test]
fn local_validation_errors_are_invalid_request_or_unsupported_file() {
    let server = Arc::new(FakeServer::default());
    let h = harness(&server, Some(TOKEN));
    let exe = mod_file(&h, "setup.exe");
    let error = h
        .invoke("community_publish_mod", request(&exe))
        .unwrap_err();
    assert_eq!(error["code"], json!("unsupported_file"));
    let zip = mod_file(&h, "a.zip");
    let mut body = request(&zip);
    body["request"]["modId"] = json!("Bad Id");
    let error = h.invoke("community_publish_mod", body).unwrap_err();
    assert_eq!(error["code"], json!("invalid_request"));
    assert!(server.seen_tokens.lock().unwrap().is_empty());
}

#[test]
fn server_refusals_map_to_stable_codes() {
    let cases = [
        (PublishApiError::VersionExists, "version_exists"),
        (PublishApiError::ModIdTaken, "mod_id_taken"),
        (PublishApiError::FileTooLarge, "file_too_large"),
        (PublishApiError::QuotaExceeded, "quota_exceeded"),
        (PublishApiError::Unauthorized, "session_expired"),
        (PublishApiError::Network("down".into()), "offline"),
        (PublishApiError::Server("500".into()), "server_error"),
        (PublishApiError::ChecksumMismatch, "checksum_mismatch"),
        (
            PublishApiError::RateLimited {
                retry_after_secs: Some(60),
            },
            "rate_limited",
        ),
    ];
    for (api_error, code) in cases {
        let server = Arc::new(FakeServer::default());
        *server.fail_create.lock().unwrap() = Some(api_error);
        let h = harness(&server, Some(TOKEN));
        let file = mod_file(&h, "a.zip");
        let error = h
            .invoke("community_publish_mod", request(&file))
            .unwrap_err();
        assert_eq!(error["code"], json!(code));
        assert_no_token(&error);
    }
}

#[test]
fn a_rate_limit_error_says_when_to_retry() {
    let server = Arc::new(FakeServer::default());
    *server.fail_create.lock().unwrap() = Some(PublishApiError::RateLimited {
        retry_after_secs: Some(60),
    });
    let h = harness(&server, Some(TOKEN));
    let file = mod_file(&h, "a.zip");
    let error = h
        .invoke("community_publish_mod", request(&file))
        .unwrap_err();
    assert!(error["message"].as_str().unwrap().contains("60"));
}

#[test]
fn cancelling_aborts_the_upload_and_a_second_publish_is_refused_meanwhile() {
    let server = Arc::new(FakeServer::default());
    let h = harness(&server, Some(TOKEN));
    let file = mod_file(&h, "a.zip");
    // While the first chunk is in flight, the "user" starts another publish and then cancels.
    let webview: Arc<Mutex<Option<WebviewWindow<MockRuntime>>>> = Arc::default();
    let refused: Arc<Mutex<Option<Value>>> = Arc::default();
    {
        let webview = webview.clone();
        let refused = refused.clone();
        let body = request(&file);
        *server.on_chunk.lock().unwrap() = Some(Box::new(move || {
            let window = webview.lock().unwrap().clone().expect("webview set");
            let second = invoke_on(&window, "community_publish_mod", body.clone());
            *refused.lock().unwrap() = second.err();
            invoke_on(&window, "community_cancel_publish", json!({})).unwrap();
        }));
    }
    *webview.lock().unwrap() = Some(h.webview());
    let error = h
        .invoke("community_publish_mod", request(&file))
        .unwrap_err();
    assert_eq!(error["code"], json!("cancelled"));
    assert!(*server.aborted.lock().unwrap());
    let refused = refused.lock().unwrap().clone().expect("second refused");
    assert_eq!(refused["code"], json!("invalid_request"));
    // The slot is free again afterwards.
    *server.on_chunk.lock().unwrap() = None;
    h.invoke("community_publish_mod", request(&file)).unwrap();
}

#[test]
fn cancelling_with_nothing_running_does_nothing() {
    let h = harness(&Arc::new(FakeServer::default()), Some(TOKEN));
    assert_eq!(
        h.invoke("community_cancel_publish", json!({})).unwrap(),
        Value::Null
    );
}

#[test]
fn my_mods_lists_every_version_with_status_and_reason() {
    let server = Arc::new(FakeServer::default());
    let h = harness(&server, Some(TOKEN));
    let mine = h.invoke("community_my_mods", json!({})).unwrap();
    assert_no_token(&mine);
    assert_eq!(mine[0]["id"], json!("brazil-livery"));
    assert_eq!(mine[0]["versions"][0]["status"], json!("rejected"));
    assert_eq!(
        mine[0]["versions"][0]["rejectionReason"],
        json!("Not allowed")
    );
    assert_eq!(mine[0]["versions"][1]["status"], json!("approved"));
}

#[test]
fn my_mods_needs_a_session() {
    let h = harness(&Arc::new(FakeServer::default()), None);
    let error = h.invoke("community_my_mods", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("not_signed_in"));
}

#[test]
fn without_an_injected_service_the_creator_is_signed_out() {
    // No publish service injected: the default state has no session, never a crash.
    let h = Harness::new();
    let error = h.invoke("community_my_mods", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("not_signed_in"));
}
