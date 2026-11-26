//! HTTPS adapter of the online catalog (issue #72): downloads the static
//! `catalog.json` and turns it into a [`RawCatalog`] of unvalidated drafts.
//!
//! This uses the **blocking** reqwest client, so callers must run
//! [`HttpCatalogSource::fetch`] off any async runtime thread (src-tauri wraps
//! it in `spawn_blocking`).
use motogp_application::{CatalogSource, CatalogSourceError, RawCatalog, RawEntry, SourceFetch};
use motogp_domain::{CatalogDownloadDraft, CatalogEntryDraft};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, ETAG, IF_NONE_MATCH, RETRY_AFTER};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

/// Raw URL of the catalog on the default branch of the project repository.
pub const DEFAULT_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/IcaroSilvaFK/motogp-launcher/main/catalog/catalog.json";

/// Settings of [`HttpCatalogSource`]. The URL is configurable so tests can
/// point at a local `http://127.0.0.1:<port>` server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCatalogConfig {
    pub url: String,
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub timeout: Duration,
    /// Largest accepted response body, in bytes.
    pub max_bytes: u64,
}

impl HttpCatalogConfig {
    /// Production defaults: the GitHub raw URL, a descriptive User-Agent,
    /// 10 s to connect, 20 s in total and a 2 MiB cap.
    pub fn github(app_version: &str) -> Self {
        Self {
            url: DEFAULT_CATALOG_URL.to_string(),
            user_agent: format!(
                "motogp-launcher/{app_version} (+https://github.com/IcaroSilvaFK/motogp-launcher)"
            ),
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(20),
            max_bytes: 2 * 1024 * 1024,
        }
    }
}

/// [`CatalogSource`] that downloads the catalog over HTTP(S) with rustls.
#[derive(Debug)]
pub struct HttpCatalogSource {
    client: Client,
    config: HttpCatalogConfig,
}

impl HttpCatalogSource {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpCatalogConfig) -> Result<Self, CatalogSourceError> {
        // reqwest is built with `rustls-no-provider`: pick ring, as the updater does.
        // An error only means a provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(config.user_agent.clone())
            .connect_timeout(config.connect_timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|e| CatalogSourceError::Network(e.to_string()))?;
        Ok(Self { client, config })
    }
}

impl CatalogSource for HttpCatalogSource {
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        let mut request = self.client.get(&self.config.url);
        if let Some(etag) = etag {
            request = request.header(IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .map_err(|e| CatalogSourceError::Network(e.to_string()))?;
        let status = response.status();
        if status == StatusCode::NOT_MODIFIED {
            return Ok(SourceFetch::NotModified);
        }
        if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
            return Err(CatalogSourceError::RateLimited {
                retry_after_secs: retry_after_secs(response.headers()),
            });
        }
        if status == StatusCode::NOT_FOUND {
            return Err(CatalogSourceError::NotPublished);
        }
        if !status.is_success() {
            return Err(CatalogSourceError::Http {
                status: status.as_u16(),
            });
        }
        let limit = self.config.max_bytes;
        if response.content_length().is_some_and(|len| len > limit) {
            return Err(CatalogSourceError::TooLarge { limit });
        }
        let new_etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        // Read at most limit + 1 bytes: one extra byte proves the body is too big.
        let mut body = Vec::new();
        response
            .take(limit.saturating_add(1))
            .read_to_end(&mut body)
            .map_err(|e| CatalogSourceError::Network(e.to_string()))?;
        if body.len() as u64 > limit {
            return Err(CatalogSourceError::TooLarge { limit });
        }
        Ok(SourceFetch::Fetched {
            catalog: parse_catalog(&body)?,
            etag: new_etag,
        })
    }
}

/// `Retry-After` as delta seconds. The HTTP-date form is ignored (`None`).
fn retry_after_secs(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Wire shape of one entry. Unknown fields are ignored here: strictness
/// lives in the JSON schema and in the strict catalog-file test.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryWire {
    id: Option<String>,
    title: Option<String>,
    author: Option<String>,
    game: Option<String>,
    category: Option<String>,
    description: Option<String>,
    version: Option<String>,
    updated_at: Option<String>,
    page_url: Option<String>,
    thumbnail_url: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
    long_description: Option<String>,
    download: Option<DownloadWire>,
}

#[derive(Debug, Deserialize)]
struct DownloadWire {
    url: Option<String>,
    size: Option<u64>,
    sha256: Option<String>,
}

impl EntryWire {
    fn into_draft(self) -> Result<CatalogEntryDraft, String> {
        fn need(value: Option<String>, name: &str) -> Result<String, String> {
            value.ok_or_else(|| format!("missing field {name}"))
        }
        let download = match self.download {
            None => None,
            Some(wire) => Some(CatalogDownloadDraft {
                url: need(wire.url, "download.url")?,
                size: wire.size,
                sha256: wire.sha256,
            }),
        };
        Ok(CatalogEntryDraft {
            id: need(self.id, "id")?,
            title: need(self.title, "title")?,
            author: need(self.author, "author")?,
            game: need(self.game, "game")?,
            category: need(self.category, "category")?,
            description: need(self.description, "description")?,
            version: need(self.version, "version")?,
            updated_at: need(self.updated_at, "updatedAt")?,
            page_url: need(self.page_url, "pageUrl")?,
            thumbnail_url: self.thumbnail_url,
            screenshots: self.screenshots,
            long_description: self.long_description,
            download,
        })
    }
}

/// Parses the catalog document. The top level must be an object with an
/// integer `schemaVersion` and an `entries` array; each entry is parsed on its
/// own, so one bad entry becomes [`RawEntry::Unparseable`] instead of failing
/// the whole catalog.
pub fn parse_catalog(bytes: &[u8]) -> Result<RawCatalog, CatalogSourceError> {
    let invalid = |detail: &str| CatalogSourceError::Invalid(detail.to_string());
    let root: Value =
        serde_json::from_slice(bytes).map_err(|e| CatalogSourceError::Invalid(e.to_string()))?;
    let object = root
        .as_object()
        .ok_or_else(|| invalid("top level is not an object"))?;
    let schema_version = object
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| invalid("schemaVersion is missing or not an integer"))?;
    let entries = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("entries is missing or not an array"))?;
    let entries = entries
        .iter()
        .map(|value| {
            match serde_json::from_value::<EntryWire>(value.clone())
                .map_err(|e| e.to_string())
                .and_then(EntryWire::into_draft)
            {
                Ok(draft) => RawEntry::Draft(draft),
                Err(reason) => RawEntry::Unparseable { reason },
            }
        })
        .collect();
    Ok(RawCatalog {
        schema_version,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    /// One canned response; `None` length means "close-delimited body".
    struct Canned {
        head: String,
        body: Vec<u8>,
    }

    /// Minimal one-shot-per-connection server; returns the URL and the raw requests seen.
    fn serve(responses: Vec<Canned>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/catalog.json", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for canned in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                log.lock().unwrap().push(read_head(&mut stream));
                let _ = stream.write_all(canned.head.as_bytes());
                let _ = stream.write_all(&canned.body);
                let _ = stream.flush();
            }
        });
        (url, seen)
    }

    fn read_head(stream: &mut TcpStream) -> String {
        let mut data = Vec::new();
        let mut byte = [0u8; 1];
        while !data.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap_or(0) == 0 {
                break;
            }
            data.push(byte[0]);
        }
        String::from_utf8_lossy(&data).to_string()
    }

    fn ok(body: &str, extra: &str) -> Canned {
        Canned {
            head: format!(
                "HTTP/1.1 200 OK\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            ),
            body: body.as_bytes().to_vec(),
        }
    }

    fn status(code: &str, extra: &str) -> Canned {
        Canned {
            head: format!(
                "HTTP/1.1 {code}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n"
            ),
            body: Vec::new(),
        }
    }

    fn source(url: String, max_bytes: u64) -> HttpCatalogSource {
        HttpCatalogSource::new(HttpCatalogConfig {
            url,
            user_agent: "test-agent/1".to_string(),
            connect_timeout: Duration::from_secs(2),
            timeout: Duration::from_secs(5),
            max_bytes,
        })
        .unwrap()
    }

    const ENTRY: &str = r#"{"id":"rebalanced-ai","title":"Rebalanced AI","author":"A","game":"MotoGP 26","category":"Gameplay","description":"d","version":"1.0","updatedAt":"2026-01-02","pageUrl":"https://example.com/m","unknown":1,"download":{"url":"https://example.com/m.zip","size":10,"sha256":"abc"}}"#;

    fn catalog(entries: &str) -> String {
        format!(r#"{{"schemaVersion":1,"entries":[{entries}]}}"#)
    }

    #[test]
    fn github_config_has_the_documented_defaults() {
        let config = HttpCatalogConfig::github("1.2.3");
        assert_eq!(config.url, DEFAULT_CATALOG_URL);
        assert_eq!(
            config.user_agent,
            "motogp-launcher/1.2.3 (+https://github.com/IcaroSilvaFK/motogp-launcher)"
        );
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.timeout, Duration::from_secs(20));
        assert_eq!(config.max_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn fetches_and_parses_with_etag_and_user_agent() {
        let (url, seen) = serve(vec![ok(&catalog(ENTRY), "ETag: \"v1\"\r\n")]);
        let result = source(url, 4096).fetch(None).unwrap();
        let SourceFetch::Fetched { catalog, etag } = result else {
            panic!("expected Fetched");
        };
        assert_eq!(etag.as_deref(), Some("\"v1\""));
        assert_eq!(catalog.schema_version, 1);
        let RawEntry::Draft(draft) = &catalog.entries[0] else {
            panic!("expected a draft");
        };
        assert_eq!(draft.id, "rebalanced-ai");
        assert_eq!(draft.download.as_ref().unwrap().size, Some(10));
        let request = seen.lock().unwrap()[0].to_lowercase();
        assert!(request.contains("user-agent: test-agent/1"));
        assert!(!request.contains("if-none-match"));
    }

    #[test]
    fn sends_if_none_match_and_maps_304() {
        let (url, seen) = serve(vec![status("304 Not Modified", "")]);
        let result = source(url, 4096).fetch(Some("\"v1\"")).unwrap();
        assert_eq!(result, SourceFetch::NotModified);
        assert!(seen.lock().unwrap()[0]
            .to_lowercase()
            .contains("if-none-match: \"v1\""));
    }

    #[test]
    fn rate_limit_reads_retry_after_seconds() {
        let (url, _) = serve(vec![status("429 Too Many Requests", "Retry-After: 60\r\n")]);
        assert_eq!(
            source(url, 4096).fetch(None),
            Err(CatalogSourceError::RateLimited {
                retry_after_secs: Some(60)
            })
        );
    }

    #[test]
    fn service_unavailable_with_http_date_has_no_retry_after() {
        let (url, _) = serve(vec![status(
            "503 Service Unavailable",
            "Retry-After: Wed, 21 Oct 2026 07:28:00 GMT\r\n",
        )]);
        assert_eq!(
            source(url, 4096).fetch(None),
            Err(CatalogSourceError::RateLimited {
                retry_after_secs: None
            })
        );
    }

    #[test]
    fn a_404_is_not_published() {
        let (url, _) = serve(vec![status("404 Not Found", "")]);
        assert_eq!(
            source(url, 4096).fetch(None),
            Err(CatalogSourceError::NotPublished)
        );
    }

    #[test]
    fn other_statuses_map_to_http_error() {
        let (url, _) = serve(vec![status("500 Internal Server Error", "")]);
        assert_eq!(
            source(url, 4096).fetch(None),
            Err(CatalogSourceError::Http { status: 500 })
        );
    }

    #[test]
    fn content_length_over_the_cap_is_too_large() {
        let (url, _) = serve(vec![ok(&catalog(ENTRY), "")]);
        assert_eq!(
            source(url, 50).fetch(None),
            Err(CatalogSourceError::TooLarge { limit: 50 })
        );
    }

    #[test]
    fn streamed_body_over_the_cap_is_too_large() {
        // No Content-Length: the body ends when the server closes the connection.
        let body = catalog(ENTRY).into_bytes();
        let canned = Canned {
            head: "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_string(),
            body,
        };
        let (url, _) = serve(vec![canned]);
        assert_eq!(
            source(url, 50).fetch(None),
            Err(CatalogSourceError::TooLarge { limit: 50 })
        );
    }

    #[test]
    fn connection_refused_is_a_network_error() {
        // Bind then drop to get a port nobody listens on.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let result = source(format!("http://127.0.0.1:{port}/c.json"), 4096).fetch(None);
        assert!(matches!(result, Err(CatalogSourceError::Network(_))));
    }

    #[test]
    fn invalid_json_is_invalid() {
        let (url, _) = serve(vec![ok("not json", "")]);
        assert!(matches!(
            source(url, 4096).fetch(None),
            Err(CatalogSourceError::Invalid(_))
        ));
    }

    #[test]
    fn parse_rejects_wrong_top_level_shapes() {
        for body in [
            "[]",
            r#"{"entries":[]}"#,
            r#"{"schemaVersion":"1","entries":[]}"#,
            r#"{"schemaVersion":1}"#,
            r#"{"schemaVersion":1,"entries":{}}"#,
        ] {
            assert!(
                matches!(
                    parse_catalog(body.as_bytes()),
                    Err(CatalogSourceError::Invalid(_))
                ),
                "{body}"
            );
        }
    }

    #[test]
    fn parse_keeps_bad_entries_as_unparseable() {
        let body = catalog(&format!(
            r#"{ENTRY},"oops",{{"id":"x"}},{{"id":1}},{{"id":"y","title":"t","author":"a","game":"g","category":"c","description":"d","version":"1","updatedAt":"2026-01-01","pageUrl":"https://e.com","download":{{"size":1}}}}"#
        ));
        let parsed = parse_catalog(body.as_bytes()).unwrap();
        assert_eq!(parsed.entries.len(), 5);
        assert!(matches!(parsed.entries[0], RawEntry::Draft(_)));
        for entry in &parsed.entries[1..] {
            assert!(matches!(entry, RawEntry::Unparseable { .. }), "{entry:?}");
        }
        let RawEntry::Unparseable { reason } = &parsed.entries[2] else {
            unreachable!()
        };
        assert_eq!(reason, "missing field title");
    }

    #[test]
    fn parse_keeps_optional_fields() {
        let body = catalog(
            r#"{"id":"a","title":"t","author":"a","game":"g","category":"c","description":"d","version":"1","updatedAt":"2026-01-01","pageUrl":"https://e.com","thumbnailUrl":"https://e.com/t.png"}"#,
        );
        let parsed = parse_catalog(body.as_bytes()).unwrap();
        let RawEntry::Draft(draft) = &parsed.entries[0] else {
            panic!("draft expected");
        };
        assert_eq!(draft.thumbnail_url.as_deref(), Some("https://e.com/t.png"));
        assert!(draft.download.is_none());
    }

    #[test]
    fn parse_reads_screenshots_and_long_description() {
        let body = catalog(
            r#"{"id":"a","title":"t","author":"a","game":"g","category":"c","description":"d","version":"1","updatedAt":"2026-01-01","pageUrl":"https://e.com","screenshots":["https://e.com/1.png","https://e.com/2.png"],"longDescription":"Long"}"#,
        );
        let parsed = parse_catalog(body.as_bytes()).unwrap();
        let RawEntry::Draft(draft) = &parsed.entries[0] else {
            panic!("draft expected");
        };
        assert_eq!(draft.screenshots.len(), 2);
        assert_eq!(draft.long_description.as_deref(), Some("Long"));
    }
}
