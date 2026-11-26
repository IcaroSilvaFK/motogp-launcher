//! HTTP adapter of the community server's mod hosting endpoints (issue #130).
//!
//! Blocking reqwest, so callers run it off the async runtime (src-tauri wraps
//! it in `spawn_blocking`). The session token only goes into the
//! `Authorization` header: it is never logged and never part of an error text.
use crate::community_http::describe;
use crate::HttpCommunityConfig;
use motogp_application::{
    CreateUpload, ModPublishApi, ModVersion, MyMod, PublishApiError, UploadSession, UploadStatus,
    VersionStatus,
};
use motogp_domain::SessionToken;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{CONTENT_TYPE, RETRY_AFTER};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;

/// Largest accepted JSON answer. "My mods" lists every version of every mod.
const MAX_BODY_BYTES: u64 = 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateWire<'a> {
    mod_id: &'a str,
    title: &'a str,
    description: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    long_description: Option<&'a str>,
    category: &'a str,
    game: &'a str,
    version: &'a str,
    file_name: &'a str,
    size: u64,
    sha256: &'a str,
    ownership_confirmed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionWire {
    upload_id: String,
    chunk_size: u64,
    received: u64,
}

#[derive(Deserialize)]
struct ReceivedWire {
    received: u64,
}

#[derive(Deserialize)]
struct StatusWire {
    received: u64,
    size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VersionWire {
    mod_id: String,
    version: String,
    title: String,
    description: String,
    long_description: Option<String>,
    category: String,
    game: String,
    file_name: String,
    size: u64,
    sha256: String,
    status: String,
    rejection_reason: Option<String>,
    #[serde(default)]
    screenshot_count: u32,
    uploaded_at: String,
    reviewed_at: Option<String>,
}

#[derive(Deserialize)]
struct MineWire {
    mods: Vec<MyModWire>,
}

#[derive(Deserialize)]
struct MyModWire {
    id: String,
    versions: Vec<VersionWire>,
}

impl TryFrom<VersionWire> for ModVersion {
    type Error = PublishApiError;

    fn try_from(wire: VersionWire) -> Result<Self, Self::Error> {
        let status = match wire.status.as_str() {
            "pending" => VersionStatus::Pending,
            "approved" => VersionStatus::Approved,
            "rejected" => VersionStatus::Rejected,
            other => {
                return Err(PublishApiError::Server(format!(
                    "unknown version status {other}"
                )))
            }
        };
        Ok(ModVersion {
            mod_id: wire.mod_id,
            version: wire.version,
            title: wire.title,
            description: wire.description,
            long_description: wire.long_description,
            category: wire.category,
            game: wire.game,
            file_name: wire.file_name,
            size: wire.size,
            sha256: wire.sha256,
            status,
            rejection_reason: wire.rejection_reason,
            screenshot_count: wire.screenshot_count,
            uploaded_at: wire.uploaded_at,
            reviewed_at: wire.reviewed_at,
        })
    }
}

/// [`ModPublishApi`] over HTTP(S).
#[derive(Debug)]
pub struct HttpModPublishApi {
    client: Client,
    base_url: String,
}

impl HttpModPublishApi {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    /// `config.timeout` bounds one request: size it for an 8 MiB chunk.
    pub fn new(config: HttpCommunityConfig) -> Result<Self, PublishApiError> {
        // See `HttpCommunityApi::new`: an error only means a provider is already installed.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(config.user_agent)
            .connect_timeout(config.connect_timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|e| PublishApiError::Network(e.to_string()))?;
        Ok(Self {
            client,
            base_url: config.base_url,
        })
    }

    fn upload_url(&self, upload_id: &str, suffix: &str) -> Result<String, PublishApiError> {
        // Ids the server issued are plain tokens; anything else would change the path.
        let valid = !upload_id.is_empty()
            && upload_id.len() <= 100
            && upload_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !valid {
            return Err(PublishApiError::BadRequest("invalid upload id".into()));
        }
        Ok(format!(
            "{}/mods/uploads/{upload_id}{suffix}",
            self.base_url
        ))
    }

    fn send(&self, request: RequestBuilder) -> Result<Response, PublishApiError> {
        let response = request
            .send()
            .map_err(|e| PublishApiError::Network(describe(&e)))?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(error_from(response))
        }
    }
}

fn read_limited(response: Response) -> Result<Vec<u8>, PublishApiError> {
    let mut bytes = Vec::new();
    response
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| PublishApiError::Network("could not read the answer".into()))?;
    if bytes.len() as u64 > MAX_BODY_BYTES {
        return Err(PublishApiError::Server("answer too large".into()));
    }
    Ok(bytes)
}

fn read_json<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, PublishApiError> {
    serde_json::from_slice(&read_limited(response)?)
        .map_err(|_| PublishApiError::Server("answer is not the expected JSON".into()))
}

/// Classifies a non-success answer from its status and the error code in the
/// body (`{"error": {"code", "message", "received"?}}`).
fn error_from(response: Response) -> PublishApiError {
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok());
    let body: Value = read_limited(response)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    let error = &body["error"];
    let code = error["code"].as_str().unwrap_or("");
    let message = error["message"].as_str().unwrap_or("").to_string();
    classify(
        status,
        code,
        message,
        error["received"].as_u64(),
        retry_after,
    )
}

fn classify(
    status: u16,
    code: &str,
    message: String,
    received: Option<u64>,
    retry_after_secs: Option<u64>,
) -> PublishApiError {
    match (status, code) {
        (401, _) => PublishApiError::Unauthorized,
        (403, "banned") => PublishApiError::Banned,
        (403, _) => PublishApiError::Forbidden,
        (_, "file_too_large") => PublishApiError::FileTooLarge,
        (_, "quota_exceeded") => PublishApiError::QuotaExceeded,
        (429, _) | (_, "rate_limited") => PublishApiError::RateLimited { retry_after_secs },
        (_, "version_exists") => PublishApiError::VersionExists,
        (_, "mod_id_taken") => PublishApiError::ModIdTaken,
        (_, "unsupported_file") => PublishApiError::UnsupportedFile,
        (_, "checksum_mismatch") => PublishApiError::ChecksumMismatch,
        (_, "offset_mismatch") => PublishApiError::OffsetMismatch { received },
        (500..=599, _) => PublishApiError::Server(format!("HTTP {status}")),
        (_, _) => PublishApiError::BadRequest(if message.is_empty() {
            format!("HTTP {status}")
        } else {
            message
        }),
    }
}

impl ModPublishApi for HttpModPublishApi {
    fn create_upload(
        &self,
        token: &SessionToken,
        upload: &CreateUpload,
    ) -> Result<UploadSession, PublishApiError> {
        let wire = CreateWire {
            mod_id: &upload.mod_id,
            title: &upload.title,
            description: &upload.description,
            long_description: upload.long_description.as_deref(),
            category: &upload.category,
            game: &upload.game,
            version: &upload.version,
            file_name: &upload.file_name,
            size: upload.size,
            sha256: &upload.sha256,
            ownership_confirmed: upload.ownership_confirmed,
        };
        let body = serde_json::to_vec(&wire)
            .map_err(|_| PublishApiError::BadRequest("could not encode the request".into()))?;
        let response = self.send(
            self.client
                .post(format!("{}/mods/uploads", self.base_url))
                .bearer_auth(token.expose())
                .header(CONTENT_TYPE, "application/json")
                .body(body),
        )?;
        let session: SessionWire = read_json(response)?;
        Ok(UploadSession {
            upload_id: session.upload_id,
            chunk_size: session.chunk_size,
            received: session.received,
        })
    }

    fn put_chunk(
        &self,
        token: &SessionToken,
        upload_id: &str,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, PublishApiError> {
        let url = self.upload_url(upload_id, &format!("/chunks?offset={offset}"))?;
        let response = self.send(
            self.client
                .put(url)
                .bearer_auth(token.expose())
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(bytes.to_vec()),
        )?;
        Ok(read_json::<ReceivedWire>(response)?.received)
    }

    fn upload_status(
        &self,
        token: &SessionToken,
        upload_id: &str,
    ) -> Result<UploadStatus, PublishApiError> {
        let url = self.upload_url(upload_id, "")?;
        let response = self.send(self.client.get(url).bearer_auth(token.expose()))?;
        let status: StatusWire = read_json(response)?;
        Ok(UploadStatus {
            received: status.received,
            size: status.size,
        })
    }

    fn add_screenshot(
        &self,
        token: &SessionToken,
        upload_id: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), PublishApiError> {
        let url = self.upload_url(upload_id, "/screenshots")?;
        self.send(
            self.client
                .post(url)
                .bearer_auth(token.expose())
                .header(CONTENT_TYPE, content_type)
                .body(bytes.to_vec()),
        )?;
        Ok(())
    }

    fn complete(
        &self,
        token: &SessionToken,
        upload_id: &str,
    ) -> Result<ModVersion, PublishApiError> {
        let url = self.upload_url(upload_id, "/complete")?;
        let response = self.send(self.client.post(url).bearer_auth(token.expose()))?;
        read_json::<VersionWire>(response)?.try_into()
    }

    fn abort(&self, token: &SessionToken, upload_id: &str) -> Result<(), PublishApiError> {
        let url = self.upload_url(upload_id, "")?;
        self.send(self.client.delete(url).bearer_auth(token.expose()))?;
        Ok(())
    }

    fn my_mods(&self, token: &SessionToken) -> Result<Vec<MyMod>, PublishApiError> {
        let response = self.send(
            self.client
                .get(format!("{}/mods/mine", self.base_url))
                .bearer_auth(token.expose()),
        )?;
        let mine: MineWire = read_json(response)?;
        mine.mods
            .into_iter()
            .map(|m| {
                Ok(MyMod {
                    id: m.id,
                    versions: m
                        .versions
                        .into_iter()
                        .map(ModVersion::try_from)
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    /// A request as the server saw it.
    #[derive(Debug, Clone)]
    struct Seen {
        line: String,
        head: String,
        body: Vec<u8>,
    }

    impl Seen {
        fn header(&self, name: &str) -> Option<String> {
            self.head.lines().find_map(|l| {
                let (n, v) = l.split_once(':')?;
                n.trim()
                    .eq_ignore_ascii_case(name)
                    .then(|| v.trim().to_string())
            })
        }
    }

    /// Serves the scripted replies in order, one per connection, reading the
    /// whole request (head and `Content-Length` body) before answering.
    fn serve(
        replies: Vec<(&'static str, &'static str, String)>,
    ) -> (String, Arc<Mutex<Vec<Seen>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        thread::spawn(move || {
            for (status, extra, body) in replies {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut raw = Vec::new();
                let mut chunk = [0u8; 4096];
                let head_end = loop {
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break raw.len();
                    }
                    raw.extend_from_slice(&chunk[..n]);
                };
                let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while raw.len() < head_end + length {
                    let n = stream.read(&mut chunk).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&chunk[..n]);
                }
                log.lock().unwrap().push(Seen {
                    line: head.lines().next().unwrap_or_default().to_string(),
                    head,
                    body: raw[head_end.min(raw.len())..].to_vec(),
                });
                let reply = format!(
                    "HTTP/1.1 {status}\r\n{extra}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        (base, seen)
    }

    fn api(base: &str) -> HttpModPublishApi {
        HttpModPublishApi::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    fn token() -> SessionToken {
        SessionToken::new("tok-SECRET")
    }

    fn create() -> CreateUpload {
        CreateUpload {
            mod_id: "brazil-livery".into(),
            title: "Brazil Livery".into(),
            description: "d".into(),
            long_description: None,
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            version: "1.0".into(),
            file_name: "Brazil Livery.zip".into(),
            size: 12,
            sha256: "ab".repeat(32),
            ownership_confirmed: true,
        }
    }

    const VERSION: &str = r#"{"modId":"brazil-livery","version":"1.0","title":"Brazil Livery","description":"d","longDescription":null,"category":"liveries","game":"MotoGP 26","fileName":"Brazil Livery.zip","size":12,"sha256":"h","status":"pending","rejectionReason":null,"screenshotCount":2,"uploadedAt":"2026-11-26T10:00:00Z","reviewedAt":null}"#;

    fn error_body(code: &str, extra: &str) -> String {
        format!(r#"{{"error":{{"code":"{code}","message":"msg {code}"{extra}}}}}"#)
    }

    #[test]
    fn create_upload_posts_camel_case_json_with_the_bearer_token() {
        let (base, seen) = serve(vec![(
            "201 Created",
            "",
            r#"{"uploadId":"up-1","chunkSize":8388608,"received":0,"expiresAt":"x"}"#.into(),
        )]);
        let session = api(&base).create_upload(&token(), &create()).unwrap();
        assert_eq!(session.upload_id, "up-1");
        assert_eq!(session.chunk_size, 8_388_608);
        assert_eq!(session.received, 0);
        let request = &seen.lock().unwrap()[0];
        assert_eq!(request.line, "POST /mods/uploads HTTP/1.1");
        assert_eq!(
            request.header("authorization").as_deref(),
            Some("Bearer tok-SECRET")
        );
        assert_eq!(
            request.header("content-type").as_deref(),
            Some("application/json")
        );
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["modId"], "brazil-livery");
        assert_eq!(body["fileName"], "Brazil Livery.zip");
        assert_eq!(body["ownershipConfirmed"], true);
        assert_eq!(body["size"], 12);
        assert!(body.get("longDescription").is_none());
    }

    #[test]
    fn the_long_description_is_sent_when_present() {
        let (base, seen) = serve(vec![(
            "201 Created",
            "",
            r#"{"uploadId":"u","chunkSize":1,"received":0}"#.into(),
        )]);
        let mut upload = create();
        upload.long_description = Some("Long".into());
        api(&base).create_upload(&token(), &upload).unwrap();
        let body: Value = serde_json::from_slice(&seen.lock().unwrap()[0].body).unwrap();
        assert_eq!(body["longDescription"], "Long");
    }

    #[test]
    fn put_chunk_sends_raw_bytes_at_the_offset() {
        let (base, seen) = serve(vec![("200 OK", "", r#"{"received":15}"#.into())]);
        let now = api(&base)
            .put_chunk(&token(), "up-1", 10, &[1, 2, 3, 4, 5])
            .unwrap();
        assert_eq!(now, 15);
        let request = &seen.lock().unwrap()[0];
        assert_eq!(
            request.line,
            "PUT /mods/uploads/up-1/chunks?offset=10 HTTP/1.1"
        );
        assert_eq!(request.body, vec![1, 2, 3, 4, 5]);
        assert_eq!(
            request.header("content-type").as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(
            request.header("authorization").as_deref(),
            Some("Bearer tok-SECRET")
        );
    }

    #[test]
    fn upload_status_reads_the_resume_point() {
        let (base, seen) = serve(vec![(
            "200 OK",
            "",
            r#"{"received":7,"size":12,"state":"uploading"}"#.into(),
        )]);
        let status = api(&base).upload_status(&token(), "up-1").unwrap();
        assert_eq!((status.received, status.size), (7, 12));
        assert_eq!(
            seen.lock().unwrap()[0].line,
            "GET /mods/uploads/up-1 HTTP/1.1"
        );
    }

    #[test]
    fn screenshots_go_as_raw_bytes_with_their_content_type() {
        let (base, seen) = serve(vec![("201 Created", "", r#"{"index":0}"#.into())]);
        api(&base)
            .add_screenshot(&token(), "up-1", "image/png", &[9, 9])
            .unwrap();
        let request = &seen.lock().unwrap()[0];
        assert_eq!(request.line, "POST /mods/uploads/up-1/screenshots HTTP/1.1");
        assert_eq!(request.header("content-type").as_deref(), Some("image/png"));
        assert_eq!(request.body, vec![9, 9]);
    }

    #[test]
    fn complete_returns_the_pending_version() {
        let (base, seen) = serve(vec![("201 Created", "", VERSION.into())]);
        let version = api(&base).complete(&token(), "up-1").unwrap();
        assert_eq!(version.mod_id, "brazil-livery");
        assert_eq!(version.status, VersionStatus::Pending);
        assert_eq!(version.screenshot_count, 2);
        assert_eq!(
            seen.lock().unwrap()[0].line,
            "POST /mods/uploads/up-1/complete HTTP/1.1"
        );
    }

    #[test]
    fn abort_deletes_the_upload() {
        let (base, seen) = serve(vec![("204 No Content", "", String::new())]);
        api(&base).abort(&token(), "up-1").unwrap();
        assert_eq!(
            seen.lock().unwrap()[0].line,
            "DELETE /mods/uploads/up-1 HTTP/1.1"
        );
    }

    #[test]
    fn my_mods_lists_every_version_with_status_and_reason() {
        let rejected = VERSION.replace("\"pending\"", "\"rejected\"").replace(
            "\"rejectionReason\":null",
            "\"rejectionReason\":\"Not allowed\"",
        );
        let approved = VERSION.replace("\"pending\"", "\"approved\"");
        let body =
            format!(r#"{{"mods":[{{"id":"brazil-livery","versions":[{rejected},{approved}]}}]}}"#);
        let (base, seen) = serve(vec![("200 OK", "", body)]);
        let mods = api(&base).my_mods(&token()).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].versions[0].status, VersionStatus::Rejected);
        assert_eq!(
            mods[0].versions[0].rejection_reason.as_deref(),
            Some("Not allowed")
        );
        assert_eq!(mods[0].versions[1].status, VersionStatus::Approved);
        assert_eq!(seen.lock().unwrap()[0].line, "GET /mods/mine HTTP/1.1");
    }

    #[test]
    fn an_unknown_status_is_a_server_error() {
        let body = format!(
            r#"{{"mods":[{{"id":"a","versions":[{}]}}]}}"#,
            VERSION.replace("\"pending\"", "\"weird\"")
        );
        let (base, _) = serve(vec![("200 OK", "", body)]);
        assert!(matches!(
            api(&base).my_mods(&token()),
            Err(PublishApiError::Server(_))
        ));
    }

    #[test]
    fn server_codes_map_to_typed_errors() {
        let cases: Vec<(&'static str, &'static str, String, PublishApiError)> = vec![
            (
                "401 Unauthorized",
                "",
                error_body("unauthorized", ""),
                PublishApiError::Unauthorized,
            ),
            (
                "403 Forbidden",
                "",
                error_body("forbidden", ""),
                PublishApiError::Forbidden,
            ),
            (
                "403 Forbidden",
                "",
                error_body("banned", ""),
                PublishApiError::Banned,
            ),
            (
                "413 Payload Too Large",
                "",
                error_body("file_too_large", ""),
                PublishApiError::FileTooLarge,
            ),
            (
                "413 Payload Too Large",
                "",
                error_body("quota_exceeded", ""),
                PublishApiError::QuotaExceeded,
            ),
            (
                "429 Too Many Requests",
                "Retry-After: 120\r\n",
                error_body("rate_limited", ""),
                PublishApiError::RateLimited {
                    retry_after_secs: Some(120),
                },
            ),
            (
                "409 Conflict",
                "",
                error_body("version_exists", ""),
                PublishApiError::VersionExists,
            ),
            (
                "409 Conflict",
                "",
                error_body("mod_id_taken", ""),
                PublishApiError::ModIdTaken,
            ),
            (
                "400 Bad Request",
                "",
                error_body("unsupported_file", ""),
                PublishApiError::UnsupportedFile,
            ),
            (
                "400 Bad Request",
                "",
                error_body("checksum_mismatch", ""),
                PublishApiError::ChecksumMismatch,
            ),
            (
                "409 Conflict",
                "",
                error_body("offset_mismatch", ",\"received\":4096"),
                PublishApiError::OffsetMismatch {
                    received: Some(4096),
                },
            ),
            (
                "400 Bad Request",
                "",
                error_body("bad_request", ""),
                PublishApiError::BadRequest("msg bad_request".into()),
            ),
            (
                "404 Not Found",
                "",
                "not json".into(),
                PublishApiError::BadRequest("HTTP 404".into()),
            ),
            (
                "500 Internal Server Error",
                "",
                String::new(),
                PublishApiError::Server("HTTP 500".into()),
            ),
            (
                "503 Service Unavailable",
                "",
                String::new(),
                PublishApiError::Server("HTTP 503".into()),
            ),
        ];
        for (status, extra, body, expected) in cases {
            let (base, _) = serve(vec![(status, extra, body)]);
            let result = api(&base).upload_status(&token(), "up-1");
            assert_eq!(result, Err(expected), "{status}");
        }
    }

    #[test]
    fn a_rate_limit_without_retry_after_has_no_delay() {
        let (base, _) = serve(vec![(
            "429 Too Many Requests",
            "",
            error_body("rate_limited", ""),
        )]);
        assert_eq!(
            api(&base).upload_status(&token(), "up-1"),
            Err(PublishApiError::RateLimited {
                retry_after_secs: None
            })
        );
    }

    #[test]
    fn a_dead_server_is_a_network_error() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let result = api(&format!("http://127.0.0.1:{port}")).my_mods(&token());
        assert!(matches!(result, Err(PublishApiError::Network(_))));
    }

    #[test]
    fn junk_answers_are_server_errors_and_oversized_ones_too() {
        let (base, _) = serve(vec![("200 OK", "", "not json".into())]);
        assert!(matches!(
            api(&base).upload_status(&token(), "up-1"),
            Err(PublishApiError::Server(_))
        ));
        let huge = format!(
            r#"{{"received":1,"size":2,"pad":"{}"}}"#,
            "x".repeat(1024 * 1024)
        );
        let (base, _) = serve(vec![("200 OK", "", huge)]);
        assert!(matches!(
            api(&base).upload_status(&token(), "up-1"),
            Err(PublishApiError::Server(_))
        ));
    }

    #[test]
    fn unsafe_upload_ids_never_reach_the_network() {
        let api = api("http://127.0.0.1:9");
        for bad in ["", "../x", "a/b", "a?b", &"a".repeat(101)] {
            assert!(matches!(
                api.upload_status(&token(), bad),
                Err(PublishApiError::BadRequest(_))
            ));
        }
    }

    #[test]
    fn the_token_never_appears_in_errors() {
        let (base, _) = serve(vec![("500 Internal Server Error", "", String::new())]);
        let error = api(&base).my_mods(&token()).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("SECRET"));
    }
}
