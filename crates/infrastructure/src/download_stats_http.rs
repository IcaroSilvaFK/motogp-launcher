//! HTTP adapter of the anonymous download counts (issue #120). Blocking
//! reqwest, same base URL and timeouts as the community account. The install
//! id only goes into the request body: it is never logged and never part of an
//! error message.
use crate::community_http::HttpCommunityConfig;
use motogp_application::{DailyDownloads, DownloadStatsApi, DownloadStatsError, ModDownloads};
use reqwest::blocking::{Client, Response};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use std::io::Read;

/// Largest accepted server answer, in bytes.
const MAX_BODY_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
struct TotalsWire {
    mods: Vec<ModWire>,
}

#[derive(Deserialize)]
struct ModWire {
    id: String,
    total: u64,
    last_30_days: u64,
}

#[derive(Deserialize)]
struct DailyWire {
    days: Vec<DayWire>,
}

#[derive(Deserialize)]
struct DayWire {
    date: String,
    count: u64,
}

/// [`DownloadStatsApi`] over HTTP(S).
#[derive(Debug)]
pub struct HttpDownloadStats {
    client: Client,
    base_url: String,
}

fn error(text: &str) -> DownloadStatsError {
    DownloadStatsError(text.to_string())
}

impl HttpDownloadStats {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpCommunityConfig) -> Result<Self, DownloadStatsError> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(config.user_agent)
            .connect_timeout(config.connect_timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|_| error("could not build the HTTP client"))?;
        Ok(Self {
            client,
            base_url: config.base_url,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn checked(response: Result<Response, reqwest::Error>) -> Result<Response, DownloadStatsError> {
        let response = response.map_err(|_| error("the server could not be reached"))?;
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            Err(DownloadStatsError(format!("HTTP {}", status.as_u16())))
        }
    }

    fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, DownloadStatsError> {
        let response = Self::checked(self.client.get(self.url(path)).send())?;
        let mut bytes = Vec::new();
        response
            .take(MAX_BODY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| error("could not read the answer"))?;
        if bytes.len() as u64 > MAX_BODY_BYTES {
            return Err(error("answer too large"));
        }
        serde_json::from_slice(&bytes).map_err(|_| error("answer is not the expected JSON"))
    }
}

impl DownloadStatsApi for HttpDownloadStats {
    fn report(&self, mod_id: &str, install_id: &str) -> Result<(), DownloadStatsError> {
        let body = serde_json::json!({ "install_id": install_id });
        Self::checked(
            self.client
                .post(self.url(&format!("/mods/{mod_id}/downloads")))
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string())
                .send(),
        )
        .map(|_| ())
    }

    fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError> {
        let wire: TotalsWire = self.get_json("/mods/downloads")?;
        Ok(wire
            .mods
            .into_iter()
            .map(|m| ModDownloads {
                id: m.id,
                total: m.total,
                last_30_days: m.last_30_days,
            })
            .collect())
    }

    fn daily(&self, mod_id: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError> {
        let wire: DailyWire = self.get_json(&format!("/mods/{mod_id}/downloads/daily"))?;
        Ok(wire
            .days
            .into_iter()
            .map(|d| DailyDownloads {
                date: d.date,
                count: d.count,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// Serves one canned response on 127.0.0.1 and reports the raw request.
    fn serve_once(status_line: &str, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap();
                raw.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(split) = text.find("\r\n\r\n") {
                    let length = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if raw.len() >= split + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&raw).to_string());
            let _ = stream.write_all(response.as_bytes());
        });
        (base, rx)
    }

    fn api(base: &str) -> HttpDownloadStats {
        HttpDownloadStats::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    #[test]
    fn report_posts_the_install_id_to_the_mod_route() {
        let (base, requests) = serve_once("204 No Content", "");
        api(&base).report("livery-pack", "abc_DEF-123").unwrap();
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /mods/livery-pack/downloads "));
        assert!(request.contains(r#"{"install_id":"abc_DEF-123"}"#));
    }

    #[test]
    fn report_with_a_429_is_an_error() {
        let (base, _) = serve_once(
            "429 Too Many Requests",
            r#"{"error":{"code":"rate_limited","message":"x"}}"#,
        );
        assert_eq!(
            api(&base).report("a", "i").unwrap_err().0,
            "HTTP 429".to_string()
        );
    }

    #[test]
    fn totals_parse_the_mods_list() {
        let (base, requests) = serve_once(
            "200 OK",
            r#"{"mods":[{"id":"a","total":101,"last_30_days":7},{"id":"b","total":1,"last_30_days":0}]}"#,
        );
        let totals = api(&base).totals().unwrap();
        assert_eq!(
            totals,
            vec![
                ModDownloads {
                    id: "a".into(),
                    total: 101,
                    last_30_days: 7
                },
                ModDownloads {
                    id: "b".into(),
                    total: 1,
                    last_30_days: 0
                },
            ]
        );
        assert!(requests.recv().unwrap().starts_with("GET /mods/downloads "));
    }

    #[test]
    fn totals_with_a_malformed_answer_is_an_error() {
        let (base, _) = serve_once("200 OK", r#"{"nope":true}"#);
        assert!(api(&base).totals().is_err());
    }

    #[test]
    fn daily_parses_the_days_and_ignores_the_echoed_id() {
        let (base, requests) = serve_once(
            "200 OK",
            r#"{"id":"a","days":[{"date":"2026-10-28","count":0},{"date":"2026-10-29","count":3}]}"#,
        );
        let days = api(&base).daily("a").unwrap();
        assert_eq!(
            days,
            vec![
                DailyDownloads {
                    date: "2026-10-28".into(),
                    count: 0
                },
                DailyDownloads {
                    date: "2026-10-29".into(),
                    count: 3
                },
            ]
        );
        assert!(requests
            .recv()
            .unwrap()
            .starts_with("GET /mods/a/downloads/daily "));
    }

    #[test]
    fn a_server_error_is_an_error() {
        let (base, _) = serve_once("502 Bad Gateway", "{}");
        assert_eq!(api(&base).totals().unwrap_err().0, "HTTP 502".to_string());
    }

    #[test]
    fn a_closed_port_is_an_error_without_the_install_id() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let failure = api(&base).report("a", "install-SECRET").unwrap_err();
        assert!(!format!("{failure:?} {failure}").contains("install-SECRET"));
    }

    #[test]
    fn an_oversized_answer_is_rejected() {
        let big = format!(r#"{{"mods":[{{"id":"{}"}}]}}"#, "x".repeat(70 * 1024));
        let (base, _) = serve_once("200 OK", &big);
        assert_eq!(
            api(&base).totals().unwrap_err().0,
            "answer too large".to_string()
        );
    }
}
