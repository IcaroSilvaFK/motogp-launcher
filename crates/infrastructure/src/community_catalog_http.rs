//! HTTP adapter of the catalog the community server hosts (issue #130): the
//! same document as the GitHub one, at `{base}/catalog`.
//!
//! It is a thin layer over [`HttpCatalogSource`], so parsing, the ETag
//! handshake, the body cap and the error mapping are exactly the GitHub ones.
use crate::{HttpCatalogConfig, HttpCatalogSource, HttpCommunityConfig};
use motogp_application::{CatalogSource, CatalogSourceError, SourceFetch};
use std::time::Duration;

/// Largest accepted catalog, in bytes (same as the GitHub catalog).
const MAX_CATALOG_BYTES: u64 = 2 * 1024 * 1024;

/// [`CatalogSource`] over the community server's `GET /catalog`.
#[derive(Debug)]
pub struct HttpCommunityCatalogSource {
    inner: HttpCatalogSource,
}

impl HttpCommunityCatalogSource {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    /// Timeouts are short (5 s to connect, 10 s in total): when the community
    /// server is down the launcher must not wait long before showing GitHub only.
    pub fn new(config: HttpCommunityConfig) -> Result<Self, CatalogSourceError> {
        let inner = HttpCatalogSource::new(HttpCatalogConfig {
            url: format!("{}/catalog", config.base_url),
            user_agent: config.user_agent,
            connect_timeout: config.connect_timeout.min(Duration::from_secs(5)),
            timeout: config.timeout.min(Duration::from_secs(10)),
            max_bytes: MAX_CATALOG_BYTES,
        })?;
        Ok(Self { inner })
    }
}

impl CatalogSource for HttpCommunityCatalogSource {
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        self.inner.fetch(etag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::RawEntry;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    fn serve_once(status_line: &str, extra: &str, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        let response = format!(
            "HTTP/1.1 {status_line}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0u8; 4096];
            while !String::from_utf8_lossy(&raw).contains("\r\n\r\n") {
                let n = stream.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..n]);
            }
            let _ = tx.send(String::from_utf8_lossy(&raw).to_string());
            let _ = stream.write_all(response.as_bytes());
        });
        (base, rx)
    }

    fn source(base: &str) -> HttpCommunityCatalogSource {
        HttpCommunityCatalogSource::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    const ENTRY: &str = r#"{"id":"brazil-livery","title":"Brazil Livery","author":"A","game":"MotoGP 26","category":"liveries","description":"d","version":"1.0","updatedAt":"2026-11-26","pageUrl":"https://example.com/mods/brazil-livery","download":{"url":"https://example.com/files/brazil-livery/1.0/a.zip","size":10,"sha256":"abc"}}"#;

    #[test]
    fn fetches_the_catalog_endpoint_and_parses_it_with_the_etag() {
        let body = format!(r#"{{"schemaVersion":1,"entries":[{ENTRY}]}}"#);
        let (base, seen) = serve_once("200 OK", "ETag: \"c1\"\r\n", &body);
        let result = source(&base).fetch(None).unwrap();
        let SourceFetch::Fetched { catalog, etag } = result else {
            panic!("expected Fetched");
        };
        assert_eq!(etag.as_deref(), Some("\"c1\""));
        let RawEntry::Draft(draft) = &catalog.entries[0] else {
            panic!("expected a draft");
        };
        assert_eq!(draft.id, "brazil-livery");
        let request = seen.recv().unwrap();
        assert!(request.starts_with("GET /catalog HTTP/1.1"));
        assert!(!request.to_lowercase().contains("if-none-match"));
    }

    #[test]
    fn a_matching_etag_answers_not_modified() {
        let (base, seen) = serve_once("304 Not Modified", "", "");
        assert_eq!(
            source(&base).fetch(Some("\"c1\"")).unwrap(),
            SourceFetch::NotModified
        );
        assert!(seen
            .recv()
            .unwrap()
            .to_lowercase()
            .contains("if-none-match: \"c1\""));
    }

    #[test]
    fn the_github_error_mapping_applies() {
        let (base, _) = serve_once("503 Service Unavailable", "Retry-After: 30\r\n", "");
        assert_eq!(
            source(&base).fetch(None),
            Err(CatalogSourceError::RateLimited {
                retry_after_secs: Some(30)
            })
        );
        let (base, _) = serve_once("200 OK", "", "not json");
        assert!(matches!(
            source(&base).fetch(None),
            Err(CatalogSourceError::Invalid(_))
        ));
        let (base, _) = serve_once("500 Internal Server Error", "", "");
        assert_eq!(
            source(&base).fetch(None),
            Err(CatalogSourceError::Http { status: 500 })
        );
    }

    #[test]
    fn an_oversized_catalog_is_refused() {
        let body = format!(
            r#"{{"schemaVersion":1,"entries":[],"pad":"{}"}}"#,
            "x".repeat(2 * 1024 * 1024)
        );
        let (base, _) = serve_once("200 OK", "", &body);
        assert!(matches!(
            source(&base).fetch(None),
            Err(CatalogSourceError::TooLarge { .. })
        ));
    }

    #[test]
    fn an_unreachable_server_is_a_network_error() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(matches!(
            source(&format!("http://127.0.0.1:{port}")).fetch(None),
            Err(CatalogSourceError::Network(_))
        ));
    }
}
