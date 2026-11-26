//! HTTP adapter of the community server's account endpoints (issue #122).
//!
//! Blocking reqwest client with short timeouts, so callers run it off any async
//! runtime thread (src-tauri wraps it in `spawn_blocking`). The session token
//! only ever goes into the `Authorization` header: it is never logged and never
//! part of an error message.
use motogp_application::{CommunityApi, CommunityApiError, ServerUser};
use motogp_domain::SessionToken;
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use serde::Deserialize;
use std::io::Read;
use std::time::Duration;

/// Largest accepted server answer, in bytes.
pub(crate) const MAX_BODY_BYTES: u64 = 64 * 1024;

/// Settings of [`HttpCommunityApi`]. The base URL is configurable so tests can
/// point at a local `http://127.0.0.1:<port>` server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCommunityConfig {
    /// Server root without a trailing slash, for example `http://127.0.0.1:8787`.
    pub base_url: String,
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub timeout: Duration,
}

impl HttpCommunityConfig {
    /// 5 s to connect and 10 s in total: the account row must not hang.
    pub fn new(base_url: &str, app_version: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            user_agent: format!("motogp-launcher/{app_version}"),
            connect_timeout: Duration::from_secs(5),
            timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Deserialize)]
struct UserWire {
    display_name: String,
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct TokenWire {
    token: String,
    user: UserWire,
}

impl From<UserWire> for ServerUser {
    fn from(user: UserWire) -> Self {
        ServerUser {
            display_name: user.display_name,
            avatar_url: user.avatar_url,
        }
    }
}

/// Percent-encodes everything except the RFC 3986 unreserved characters.
pub(crate) fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// [`CommunityApi`] over HTTP(S).
#[derive(Debug)]
pub struct HttpCommunityApi {
    pub(crate) client: Client,
    base_url: String,
}

impl HttpCommunityApi {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpCommunityConfig) -> Result<Self, CommunityApiError> {
        // reqwest is built with `rustls-no-provider`: pick ring, as the other
        // adapters do. An error only means a provider is already installed.
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

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn send(&self, request: RequestBuilder) -> Result<Response, CommunityApiError> {
        request
            .send()
            .map_err(|e| CommunityApiError::Unreachable(describe(&e)))
    }
}

/// An error text without the URL (it may carry query strings).
pub(crate) fn describe(error: &reqwest::Error) -> String {
    let kind = if error.is_timeout() {
        "timed out"
    } else if error.is_connect() {
        "could not connect"
    } else {
        "request failed"
    };
    kind.to_string()
}

/// Maps a non-success status to the port's error type.
pub(crate) fn status_error(status: StatusCode) -> CommunityApiError {
    if status == StatusCode::FORBIDDEN {
        CommunityApiError::Banned
    } else if status == StatusCode::UNAUTHORIZED {
        CommunityApiError::Unauthorized
    } else if status.is_server_error() {
        CommunityApiError::Unreachable(format!("HTTP {}", status.as_u16()))
    } else {
        CommunityApiError::Protocol(format!("HTTP {}", status.as_u16()))
    }
}

/// Reads at most [`MAX_BODY_BYTES`] and parses JSON.
pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(
    response: Response,
) -> Result<T, CommunityApiError> {
    let mut bytes = Vec::new();
    response
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CommunityApiError::Unreachable("could not read the answer".into()))?;
    if bytes.len() as u64 > MAX_BODY_BYTES {
        return Err(CommunityApiError::Protocol("answer too large".into()));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| CommunityApiError::Protocol("answer is not the expected JSON".into()))
}

impl CommunityApi for HttpCommunityApi {
    fn start_url(&self, redirect: &str, state: &str, code_challenge: &str) -> String {
        format!(
            "{}?redirect={}&state={}&code_challenge={}",
            self.url("/auth/discord/start"),
            percent_encode(redirect),
            percent_encode(state),
            percent_encode(code_challenge),
        )
    }

    fn exchange(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<(SessionToken, ServerUser), CommunityApiError> {
        let body = serde_json::json!({ "code": code, "code_verifier": code_verifier });
        let response = self.send(
            self.client
                .post(self.url("/auth/token"))
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_string()),
        )?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        let wire: TokenWire = read_json(response)?;
        if wire.token.is_empty() {
            return Err(CommunityApiError::Protocol("empty token".into()));
        }
        Ok((SessionToken::new(wire.token), wire.user.into()))
    }

    fn me(&self, token: &SessionToken) -> Result<ServerUser, CommunityApiError> {
        let response = self.send(self.client.get(self.url("/me")).bearer_auth(token.expose()))?;
        if !response.status().is_success() {
            return Err(status_error(response.status()));
        }
        Ok(read_json::<UserWire>(response)?.into())
    }

    fn logout(&self, token: &SessionToken) -> Result<(), CommunityApiError> {
        let response = self.send(
            self.client
                .post(self.url("/auth/logout"))
                .bearer_auth(token.expose()),
        )?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(status_error(response.status()))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// Serves one canned response on 127.0.0.1 and reports the raw request.
    pub(crate) fn serve_once(status_line: &str, body: &str) -> (String, mpsc::Receiver<String>) {
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

    fn api(base: &str) -> HttpCommunityApi {
        HttpCommunityApi::new(HttpCommunityConfig::new(base, "0.0.0")).unwrap()
    }

    const TOKEN_JSON: &str = r#"{"token":"tok-123","expires_at":"2030-01-01T00:00:00Z","user":{"id":"u","discord_id":"1","username":"r","display_name":"rider","avatar_url":"https://cdn.example/a.png"}}"#;

    #[test]
    fn percent_encoding_keeps_unreserved_and_escapes_the_rest() {
        assert_eq!(
            percent_encode("http://127.0.0.1:5/callback"),
            "http%3A%2F%2F127.0.0.1%3A5%2Fcallback"
        );
        assert_eq!(percent_encode("Az09-._~"), "Az09-._~");
        assert_eq!(percent_encode("a b&c"), "a%20b%26c");
    }

    #[test]
    fn the_start_url_carries_the_encoded_query() {
        let api = api("http://127.0.0.1:8787/");
        assert_eq!(
            api.start_url("http://127.0.0.1:5555/callback", "st", "ch"),
            "http://127.0.0.1:8787/auth/discord/start?redirect=http%3A%2F%2F127.0.0.1%3A5555%2Fcallback&state=st&code_challenge=ch"
        );
    }

    #[test]
    fn exchange_posts_the_code_and_verifier_and_parses_the_session() {
        let (base, requests) = serve_once("200 OK", TOKEN_JSON);
        let (token, user) = api(&base).exchange("otc", "ver").unwrap();
        assert_eq!(token.expose(), "tok-123");
        assert_eq!(user.display_name, "rider");
        assert_eq!(
            user.avatar_url.as_deref(),
            Some("https://cdn.example/a.png")
        );
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /auth/token "));
        assert!(request.contains(r#""code":"otc""#));
        assert!(request.contains(r#""code_verifier":"ver""#));
    }

    #[test]
    fn exchange_with_an_invalid_code_is_a_protocol_error() {
        let (base, _) = serve_once(
            "400 Bad Request",
            r#"{"error":{"code":"invalid_code","message":"x"}}"#,
        );
        assert!(matches!(
            api(&base).exchange("otc", "ver"),
            Err(CommunityApiError::Protocol(_))
        ));
    }

    #[test]
    fn exchange_with_a_malformed_answer_is_a_protocol_error() {
        let (base, _) = serve_once("200 OK", r#"{"nope":true}"#);
        assert!(matches!(
            api(&base).exchange("otc", "ver"),
            Err(CommunityApiError::Protocol(_))
        ));
    }

    #[test]
    fn exchange_with_an_empty_token_is_a_protocol_error() {
        let (base, _) = serve_once(
            "200 OK",
            r#"{"token":"","expires_at":"x","user":{"display_name":"r","avatar_url":null}}"#,
        );
        assert!(matches!(
            api(&base).exchange("otc", "ver"),
            Err(CommunityApiError::Protocol(_))
        ));
    }

    #[test]
    fn me_sends_the_bearer_token_and_parses_the_user() {
        let (base, requests) = serve_once(
            "200 OK",
            r#"{"id":"u","display_name":"rider","avatar_url":null}"#,
        );
        let user = api(&base).me(&SessionToken::new("tok-123")).unwrap();
        assert_eq!(user.display_name, "rider");
        assert_eq!(user.avatar_url, None);
        let request = requests.recv().unwrap();
        assert!(request.starts_with("GET /me "));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer tok-123"));
    }

    #[test]
    fn me_with_a_401_is_unauthorized() {
        let (base, _) = serve_once(
            "401 Unauthorized",
            r#"{"error":{"code":"unauthorized","message":"x"}}"#,
        );
        assert_eq!(
            api(&base).me(&SessionToken::new("t")),
            Err(CommunityApiError::Unauthorized)
        );
    }

    #[test]
    fn a_server_error_is_unreachable() {
        let (base, _) = serve_once("502 Bad Gateway", "{}");
        assert!(matches!(
            api(&base).me(&SessionToken::new("t")),
            Err(CommunityApiError::Unreachable(_))
        ));
    }

    #[test]
    fn a_closed_port_is_unreachable_and_the_error_has_no_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let error = api(&base).me(&SessionToken::new("tok-SECRET")).unwrap_err();
        assert!(matches!(error, CommunityApiError::Unreachable(_)));
        assert!(!format!("{error:?} {error}").contains("tok-SECRET"));
    }

    #[test]
    fn logout_accepts_204_and_sends_the_bearer_token() {
        let (base, requests) = serve_once("204 No Content", "");
        api(&base).logout(&SessionToken::new("tok-123")).unwrap();
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /auth/logout "));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer tok-123"));
    }

    #[test]
    fn logout_with_a_401_is_unauthorized() {
        let (base, _) = serve_once("401 Unauthorized", "{}");
        assert_eq!(
            api(&base).logout(&SessionToken::new("t")),
            Err(CommunityApiError::Unauthorized)
        );
    }

    #[test]
    fn an_oversized_answer_is_rejected() {
        let big = format!(r#"{{"display_name":"{}"}}"#, "x".repeat(70 * 1024));
        let (base, _) = serve_once("200 OK", &big);
        assert!(matches!(
            api(&base).me(&SessionToken::new("t")),
            Err(CommunityApiError::Protocol(_))
        ));
    }
}
