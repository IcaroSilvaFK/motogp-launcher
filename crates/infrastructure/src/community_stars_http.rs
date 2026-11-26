//! HTTP adapter of the community server's stars endpoints (issue #119).
//!
//! Same client settings as [`crate::HttpCommunityApi`] (base URL, timeouts, user
//! agent). The session token only goes into the `Authorization` header.
use crate::community_http::{describe, read_json, status_error};
use crate::HttpCommunityConfig;
use motogp_application::{CommunityApiError, ModStats, ModStatsApi};
use motogp_domain::SessionToken;
use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsWire {
    id: String,
    stars: u64,
    starred_by_me: Option<bool>,
}

#[derive(Deserialize)]
struct StatsListWire {
    mods: Vec<StatsWire>,
}

impl From<StatsWire> for ModStats {
    fn from(wire: StatsWire) -> Self {
        ModStats {
            id: wire.id,
            stars: wire.stars,
            starred_by_me: wire.starred_by_me,
        }
    }
}

/// [`ModStatsApi`] over HTTP(S).
#[derive(Debug)]
pub struct HttpModStatsApi {
    client: Client,
    base_url: String,
}

impl HttpModStatsApi {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpCommunityConfig) -> Result<Self, CommunityApiError> {
        // See `HttpCommunityApi::new`: an error only means a provider is already installed.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(config.user_agent)
            .connect_timeout(config.connect_timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|e| CommunityApiError::Unreachable(e.to_string()))?;
        Ok(Self {
            client,
            base_url: config.base_url,
        })
    }

    fn send(&self, request: RequestBuilder) -> Result<Response, CommunityApiError> {
        let response = request
            .send()
            .map_err(|e| CommunityApiError::Unreachable(describe(&e)))?;
        if response.status().is_success() {
            Ok(response)
        } else {
            Err(status_error(response.status()))
        }
    }

    fn star_url(&self, id: &str) -> Result<String, CommunityApiError> {
        // Slugs only: anything else would change the path and the server refuses it anyway.
        let valid = !id.is_empty()
            && id.len() <= 100
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid {
            return Err(CommunityApiError::Protocol("invalid mod id".into()));
        }
        Ok(format!("{}/mods/{id}/star", self.base_url))
    }
}

impl ModStatsApi for HttpModStatsApi {
    fn stats(&self, token: Option<&SessionToken>) -> Result<Vec<ModStats>, CommunityApiError> {
        let mut request = self.client.get(format!("{}/mods/stats", self.base_url));
        if let Some(token) = token {
            request = request.bearer_auth(token.expose());
        }
        let list: StatsListWire = read_json(self.send(request)?)?;
        Ok(list.mods.into_iter().map(ModStats::from).collect())
    }

    fn star(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
        let request = self
            .client
            .put(self.star_url(id)?)
            .bearer_auth(token.expose());
        Ok(read_json::<StatsWire>(self.send(request)?)?.into())
    }

    fn unstar(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
        let request = self
            .client
            .delete(self.star_url(id)?)
            .bearer_auth(token.expose());
        Ok(read_json::<StatsWire>(self.send(request)?)?.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// Serves one canned response on 127.0.0.1 and reports the request head.
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
            // Requests here have no body: the head is the whole request.
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

    fn api(base: &str) -> HttpModStatsApi {
        HttpModStatsApi::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    fn has_bearer(request: &str, token: &str) -> bool {
        request
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {token}"))
    }

    const LIST: &str =
        r#"{"mods":[{"id":"a-mod","stars":3,"starredByMe":true},{"id":"b","stars":1}]}"#;

    #[test]
    fn anonymous_stats_send_no_authorization_and_parse_the_list() {
        let (base, requests) = serve_once("200 OK", LIST);
        let list = api(&base).stats(None).unwrap();
        assert_eq!(
            list,
            vec![
                ModStats {
                    id: "a-mod".into(),
                    stars: 3,
                    starred_by_me: Some(true)
                },
                ModStats {
                    id: "b".into(),
                    stars: 1,
                    starred_by_me: None
                },
            ]
        );
        let request = requests.recv().unwrap();
        assert!(request.starts_with("GET /mods/stats "));
        assert!(!request.to_ascii_lowercase().contains("authorization"));
    }

    #[test]
    fn stats_with_a_session_send_the_bearer_token() {
        let (base, requests) = serve_once("200 OK", r#"{"mods":[]}"#);
        let list = api(&base).stats(Some(&SessionToken::new("tok-1"))).unwrap();
        assert!(list.is_empty());
        assert!(has_bearer(&requests.recv().unwrap(), "tok-1"));
    }

    #[test]
    fn star_puts_with_the_bearer_token_and_parses_the_answer() {
        let (base, requests) =
            serve_once("200 OK", r#"{"id":"a-mod","stars":11,"starredByMe":true}"#);
        let stats = api(&base)
            .star(&SessionToken::new("tok-1"), "a-mod")
            .unwrap();
        assert_eq!(stats.stars, 11);
        assert_eq!(stats.starred_by_me, Some(true));
        let request = requests.recv().unwrap();
        assert!(request.starts_with("PUT /mods/a-mod/star "));
        assert!(has_bearer(&request, "tok-1"));
    }

    #[test]
    fn unstar_deletes_with_the_bearer_token() {
        let (base, requests) =
            serve_once("200 OK", r#"{"id":"a-mod","stars":10,"starredByMe":false}"#);
        let stats = api(&base)
            .unstar(&SessionToken::new("tok-1"), "a-mod")
            .unwrap();
        assert_eq!(stats.starred_by_me, Some(false));
        let request = requests.recv().unwrap();
        assert!(request.starts_with("DELETE /mods/a-mod/star "));
        assert!(has_bearer(&request, "tok-1"));
    }

    #[test]
    fn a_401_is_unauthorized() {
        let (base, _) = serve_once("401 Unauthorized", "{}");
        assert_eq!(
            api(&base).star(&SessionToken::new("t"), "a"),
            Err(CommunityApiError::Unauthorized)
        );
    }

    #[test]
    fn a_rate_limit_is_a_protocol_error_and_a_5xx_is_unreachable() {
        let (base, _) = serve_once("429 Too Many Requests", "{}");
        assert!(matches!(
            api(&base).stats(None),
            Err(CommunityApiError::Protocol(_))
        ));
        let (base, _) = serve_once("503 Service Unavailable", "{}");
        assert!(matches!(
            api(&base).stats(None),
            Err(CommunityApiError::Unreachable(_))
        ));
    }

    #[test]
    fn a_malformed_answer_is_a_protocol_error() {
        let (base, _) = serve_once("200 OK", r#"{"nope":1}"#);
        assert!(matches!(
            api(&base).stats(None),
            Err(CommunityApiError::Protocol(_))
        ));
    }

    #[test]
    fn a_closed_port_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        assert!(matches!(
            api(&base).stats(None),
            Err(CommunityApiError::Unreachable(_))
        ));
    }

    #[test]
    fn an_id_that_is_not_a_slug_never_reaches_the_network() {
        // Port 9 is never contacted: validation fails first.
        let api = api("http://127.0.0.1:9");
        for id in ["", "Not A Slug!", "a/b", "../x", &"a".repeat(101)] {
            assert!(
                matches!(
                    api.star(&SessionToken::new("t"), id),
                    Err(CommunityApiError::Protocol(_))
                ),
                "{id}"
            );
        }
    }
}
