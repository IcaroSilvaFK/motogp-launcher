//! Shared harness for the stars tests: real router, in-memory SQLite, fake
//! clock and fake Discord (no network, no real time).

#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request};
use axum::Router;
use community_api::testing::{auth_service, comments_state, star_service, FakeClock, FakeDiscord};
use community_api::{build_router, AppState, RateLimits};
use community_application::ports::DiscordUser;
use community_domain::pkce;
use community_infrastructure::SqliteStore;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const REDIRECT: &str = "http://127.0.0.1:54321/callback";
const APP_STATE: &str = "app-state-123";
const APP_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

pub struct Harness {
    pub app: Router,
    pub clock: Arc<FakeClock>,
    pub discord: Arc<FakeDiscord>,
}

impl Harness {
    pub async fn new() -> Self {
        Self::with_user_limit(RateLimits::USER_PER_MINUTE).await
    }

    pub async fn with_user_limit(user_per_minute: u32) -> Self {
        let clock = Arc::new(FakeClock::at_epoch_plus(1_700_000_000));
        let discord = Arc::new(FakeDiscord::new("rider"));
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        let store = Arc::new(store);
        let state = AppState {
            clock: clock.clone(),
            health: store.clone(),
            auth: auth_service(store.clone(), clock.clone(), discord.clone()),
            stars: star_service(store.clone()),
            // Generous per-IP limits: these tests are not about them.
            limits: RateLimits::with_limits(clock.clone(), 10_000, 10_000, user_per_minute),
            comments: comments_state(store, clock.clone()),
            trust_proxy_headers: false,
        };
        Self {
            app: build_router(state),
            clock,
            discord,
        }
    }

    pub async fn send(&self, method: Method, uri: &str, bearer: Option<&str>) -> Reply {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.finish(builder.body(Body::empty()).unwrap()).await
    }

    async fn finish(&self, mut request: Request<Body>) -> Reply {
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    pub async fn location(&self, uri: &str) -> String {
        let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let mut request = request;
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), 302, "{uri}");
        response.headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_owned()
    }

    /// Signs a Discord user in through the real flow and returns the session token.
    pub async fn sign_in(&self, discord_id: &str, username: &str) -> String {
        self.discord.set_user(DiscordUser {
            id: discord_id.into(),
            username: username.into(),
            global_name: None,
            avatar: None,
        });
        let start = format!(
            "/auth/discord/start?redirect={REDIRECT}&state={APP_STATE}&code_challenge={}",
            pkce::challenge_for(APP_VERIFIER)
        );
        let authorize = self.location(&start).await;
        let state = query_param(&authorize, "state").expect("Discord state");
        let loopback = self
            .location(&format!(
                "/auth/discord/callback?code=discord-code&state={state}"
            ))
            .await;
        let code = query_param(&loopback, "code").expect("one-time code");
        let request = Request::builder()
            .method(Method::POST)
            .uri("/auth/token")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "code": code, "code_verifier": APP_VERIFIER }).to_string(),
            ))
            .unwrap();
        let reply = self.finish(request).await;
        assert_eq!(reply.status, 200, "{reply:?}");
        reply.body["token"].as_str().expect("token").to_owned()
    }
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_owned())
    })
}
