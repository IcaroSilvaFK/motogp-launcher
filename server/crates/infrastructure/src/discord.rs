//! Discord OAuth2 adapter (reqwest + rustls). Never logs tokens, codes or the secret.

use std::fmt;
use std::time::Duration;

use community_application::ports::{AccessToken, DiscordOAuth, DiscordUser, PortResult};
use community_application::AppError;
use reqwest::Url;
use serde_json::Value;

const AUTHORIZE_URL: &str = "https://discord.com/oauth2/authorize";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct DiscordHttp {
    client: reqwest::Client,
    api_base: String,
    client_id: String,
    client_secret: String,
    /// `{PUBLIC_BASE_URL}/auth/discord/callback`.
    callback_url: String,
}

impl fmt::Debug for DiscordHttp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscordHttp")
            .field("api_base", &self.api_base)
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

impl DiscordHttp {
    pub fn new(
        api_base: &str,
        client_id: &str,
        client_secret: &str,
        public_base_url: &str,
    ) -> Result<Self, reqwest::Error> {
        // reqwest is built with `rustls-no-provider`: pick ring explicitly.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            client,
            api_base: api_base.trim_end_matches('/').to_owned(),
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
            callback_url: format!(
                "{}/auth/discord/callback",
                public_base_url.trim_end_matches('/')
            ),
        })
    }

    fn unavailable(what: &str, detail: impl fmt::Display) -> AppError {
        // `detail` is a status code or a transport error kind: never a body or a token.
        tracing::warn!(what, %detail, "discord request failed");
        AppError::DiscordUnavailable
    }
}

#[async_trait::async_trait]
impl DiscordOAuth for DiscordHttp {
    fn authorize_url(&self, state: &str, code_challenge: &str) -> String {
        let mut url = Url::parse(AUTHORIZE_URL).expect("constant URL is valid");
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.client_id)
            .append_pair("scope", "identify")
            .append_pair("redirect_uri", &self.callback_url)
            .append_pair("state", state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256");
        url.into()
    }

    async fn exchange_code(&self, code: &str, code_verifier: &str) -> PortResult<AccessToken> {
        let response = self
            .client
            .post(format!("{}/oauth2/token", self.api_base))
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", self.callback_url.as_str()),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| Self::unavailable("token", transport_kind(&e)))?;
        if !response.status().is_success() {
            return Err(Self::unavailable("token", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|e| Self::unavailable("token", transport_kind(&e)))?;
        body.get("access_token")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(|t| AccessToken(t.to_owned()))
            .ok_or_else(|| Self::unavailable("token", "missing access_token"))
    }

    async fn fetch_user(&self, access_token: &AccessToken) -> PortResult<DiscordUser> {
        let response = self
            .client
            .get(format!("{}/users/@me", self.api_base))
            .bearer_auth(&access_token.0)
            .send()
            .await
            .map_err(|e| Self::unavailable("user", transport_kind(&e)))?;
        if !response.status().is_success() {
            return Err(Self::unavailable("user", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|e| Self::unavailable("user", transport_kind(&e)))?;
        let text = |key: &str| {
            body.get(key)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        match (text("id"), text("username")) {
            (Some(id), Some(username)) => Ok(DiscordUser {
                id,
                username,
                global_name: text("global_name"),
                avatar: text("avatar"),
            }),
            _ => Err(Self::unavailable("user", "missing id or username")),
        }
    }
}

/// A short label for a transport error that cannot contain URLs or secrets.
fn transport_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_decode() {
        "decode"
    } else {
        "transport"
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Form, Json, Router};
    use serde_json::json;
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct Seen {
        form: Mutex<HashMap<String, String>>,
        auth: Mutex<Option<String>>,
    }

    /// Fake Discord on 127.0.0.1:0. `token_status` / `user_status` simulate failures.
    async fn serve(token_status: u16, user_status: u16) -> (String, Arc<Seen>) {
        let seen = Arc::new(Seen::default());
        let app = Router::new()
            .route(
                "/oauth2/token",
                post(
                    move |State(seen): State<Arc<Seen>>,
                          Form(form): Form<HashMap<String, String>>| async move {
                        *seen.form.lock().unwrap() = form;
                        (
                            StatusCode::from_u16(token_status).unwrap(),
                            Json(json!({"access_token": "acc-123", "token_type": "Bearer"})),
                        )
                    },
                ),
            )
            .route(
                "/users/@me",
                get(
                    move |State(seen): State<Arc<Seen>>, headers: HeaderMap| async move {
                        *seen.auth.lock().unwrap() = headers
                            .get("authorization")
                            .map(|v| v.to_str().unwrap().to_owned());
                        (
                            StatusCode::from_u16(user_status).unwrap(),
                            Json(json!({
                                "id": "1234",
                                "username": "rider",
                                "global_name": "Rider One",
                                "avatar": "hash1",
                                "extra": true
                            })),
                        )
                    },
                ),
            )
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (base, seen)
    }

    fn adapter(base: &str) -> DiscordHttp {
        DiscordHttp::new(base, "client-1", "s3cret", "https://community.example/").unwrap()
    }

    #[test]
    fn authorize_url_has_all_parameters_and_encodes_the_redirect() {
        let url = adapter("http://x").authorize_url("st", "ch");
        let parsed = Url::parse(&url).unwrap();
        assert_eq!(parsed.host_str(), Some("discord.com"));
        assert_eq!(parsed.path(), "/oauth2/authorize");
        let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["client_id"], "client-1");
        assert_eq!(query["scope"], "identify");
        assert_eq!(
            query["redirect_uri"],
            "https://community.example/auth/discord/callback"
        );
        assert_eq!(query["state"], "st");
        assert_eq!(query["code_challenge"], "ch");
        assert_eq!(query["code_challenge_method"], "S256");
        assert!(!url.contains("s3cret"));
    }

    #[test]
    fn debug_redacts_the_secret() {
        assert!(!format!("{:?}", adapter("http://x")).contains("s3cret"));
    }

    #[tokio::test]
    async fn exchanges_the_code_with_a_form_body() {
        let (base, seen) = serve(200, 200).await;
        let token = adapter(&base)
            .exchange_code("the-code", "the-verifier")
            .await
            .unwrap();
        assert_eq!(token.0, "acc-123");
        let form = seen.form.lock().unwrap();
        assert_eq!(form["client_id"], "client-1");
        assert_eq!(form["client_secret"], "s3cret");
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "the-code");
        assert_eq!(form["code_verifier"], "the-verifier");
        assert_eq!(
            form["redirect_uri"],
            "https://community.example/auth/discord/callback"
        );
    }

    #[tokio::test]
    async fn fetches_the_user_with_a_bearer_token() {
        let (base, seen) = serve(200, 200).await;
        let user = adapter(&base)
            .fetch_user(&AccessToken("acc-123".into()))
            .await
            .unwrap();
        assert_eq!(
            user,
            DiscordUser {
                id: "1234".into(),
                username: "rider".into(),
                global_name: Some("Rider One".into()),
                avatar: Some("hash1".into()),
            }
        );
        assert_eq!(seen.auth.lock().unwrap().as_deref(), Some("Bearer acc-123"));
    }

    #[tokio::test]
    async fn non_2xx_answers_map_to_discord_unavailable() {
        let (base, _) = serve(400, 401).await;
        let discord = adapter(&base);
        assert_eq!(
            discord.exchange_code("c", "v").await.unwrap_err(),
            AppError::DiscordUnavailable
        );
        assert_eq!(
            discord
                .fetch_user(&AccessToken("t".into()))
                .await
                .unwrap_err(),
            AppError::DiscordUnavailable
        );
    }

    #[tokio::test]
    async fn transport_errors_map_to_discord_unavailable() {
        // Bind then drop a listener so the port refuses connections.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let discord = adapter(&base);
        assert_eq!(
            discord.exchange_code("c", "v").await.unwrap_err(),
            AppError::DiscordUnavailable
        );
        assert_eq!(
            discord
                .fetch_user(&AccessToken("t".into()))
                .await
                .unwrap_err(),
            AppError::DiscordUnavailable
        );
    }
}
