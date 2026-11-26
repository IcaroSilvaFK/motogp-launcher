//! BDD runner (cucumber, `harness = false`) for `features/community-auth.feature`.
//!
//! Every step drives the real router (in-memory SQLite, fake clock, fake
//! Discord). The run exits non-zero when a step fails, is missing or ambiguous.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request};
use axum::Router;
use chrono::Duration;
use community_api::testing::{auth_service, comments_state, star_service, FakeClock, FakeDiscord};
use community_api::{build_router, AppState, RateLimits};
use community_domain::pkce;
use community_infrastructure::SqliteStore;
use cucumber::{given, then, when, World};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const REDIRECT: &str = "http://127.0.0.1:54321/callback";
const APP_STATE: &str = "app-state-123";
const APP_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const CLIENT_IP: &str = "10.0.0.1";

/// What the last request answered.
#[derive(Debug, Clone)]
struct Reply {
    status: u16,
    location: Option<String>,
    body: Value,
}

struct Harness {
    app: Router,
    clock: Arc<FakeClock>,
    discord: Arc<FakeDiscord>,
    store: Arc<SqliteStore>,
}

#[derive(World, Default)]
struct AuthWorld {
    harness: Option<Harness>,
    last: Option<Reply>,
    token: Option<String>,
    code: Option<String>,
    replies: Vec<Reply>,
}

impl fmt::Debug for AuthWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthWorld")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl AuthWorld {
    async fn harness(&mut self) -> &Harness {
        if self.harness.is_none() {
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
                downloads: community_api::testing::downloads_state(store.clone(), clock.clone()),
                limits: RateLimits::new(clock.clone()),
                moderation: community_api::testing::moderation_service(
                    store.clone(),
                    clock.clone(),
                    Vec::new(),
                ),
                comments: comments_state(store.clone(), clock.clone()),
                trust_proxy_headers: false,
                mods: community_api::testing::unused_mod_hosting(store.clone(), clock.clone()),
            };
            self.harness = Some(Harness {
                app: build_router(state),
                clock,
                discord,
                store,
            });
        }
        self.harness.as_ref().unwrap()
    }

    async fn send(
        &mut self,
        method: Method,
        uri: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let app = self.harness().await.app.clone();
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request_body = match body {
            Some(value) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let mut request = builder.body(request_body).unwrap();
        let addr: SocketAddr = format!("{CLIENT_IP}:4000").parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = app.oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        let reply = Reply {
            status,
            location,
            body,
        };
        self.last = Some(reply.clone());
        reply
    }

    fn start_uri(redirect: &str) -> String {
        format!(
            "/auth/discord/start?redirect={redirect}&state={APP_STATE}&code_challenge={}",
            pkce::challenge_for(APP_VERIFIER)
        )
    }

    /// start -> Discord consent (skipped) -> callback; returns the one-time code.
    async fn obtain_code(&mut self) -> String {
        let start = self
            .send(Method::GET, &Self::start_uri(REDIRECT), None, None)
            .await;
        assert_eq!(start.status, 302, "start must redirect: {start:?}");
        let authorize = start.location.expect("start sets a Location");
        assert!(authorize.starts_with("https://discord.test/oauth2/authorize?"));
        let state = query_param(&authorize, "state").expect("Discord state");

        let callback = self
            .send(
                Method::GET,
                &format!("/auth/discord/callback?code=discord-code&state={state}"),
                None,
                None,
            )
            .await;
        assert_eq!(callback.status, 302, "callback must redirect: {callback:?}");
        let loopback = callback.location.expect("callback sets a Location");
        assert!(loopback.starts_with(&format!("{REDIRECT}?")), "{loopback}");
        assert_eq!(query_param(&loopback, "state").as_deref(), Some(APP_STATE));
        query_param(&loopback, "code").expect("one-time code")
    }

    async fn exchange(&mut self, code: &str, verifier: &str) -> Reply {
        self.send(
            Method::POST,
            "/auth/token",
            None,
            Some(json!({ "code": code, "code_verifier": verifier })),
        )
        .await
    }

    async fn sign_in(&mut self) -> String {
        let code = self.obtain_code().await;
        let reply = self.exchange(&code, APP_VERIFIER).await;
        assert_eq!(reply.status, 200, "exchange failed: {reply:?}");
        let token = reply.body["token"].as_str().expect("token").to_owned();
        self.token = Some(token.clone());
        token
    }

    fn last(&self) -> &Reply {
        self.last.as_ref().expect("a request was made")
    }
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_owned())
    })
}

fn assert_error(reply: &Reply, status: u16, code: &str) {
    assert_eq!(reply.status, status, "{reply:?}");
    assert_eq!(reply.body["error"]["code"], code, "{reply:?}");
}

// ---- Given ----

#[given(regex = r#"^Discord authorizes the user "([^"]*)"$"#)]
async fn discord_authorizes(world: &mut AuthWorld, name: String) {
    let discord = world.harness().await.discord.clone();
    discord.set_user(community_application::ports::DiscordUser {
        id: "987654321098765432".into(),
        username: name,
        global_name: None,
        avatar: None,
    });
}

#[given(regex = r"^a one-time code was already exchanged$")]
async fn code_already_exchanged(world: &mut AuthWorld) {
    let code = world.obtain_code().await;
    let reply = world.exchange(&code, APP_VERIFIER).await;
    assert_eq!(reply.status, 200);
    world.code = Some(code);
}

#[given(regex = r"^a signed-in session$")]
async fn signed_in_session(world: &mut AuthWorld) {
    world.sign_in().await;
}

#[given(regex = r"^a session created 31 days ago$")]
async fn session_31_days_ago(world: &mut AuthWorld) {
    world.sign_in().await;
    world.harness().await.clock.advance(Duration::days(31));
}

#[given(regex = r"^the per-IP limit for /auth is 10 per minute$")]
async fn auth_limit_is_ten(world: &mut AuthWorld) {
    assert_eq!(RateLimits::AUTH_PER_MINUTE, 10);
    world.harness().await;
}

// ---- When ----

#[when(regex = r"^a client calls GET /health$")]
async fn get_health(world: &mut AuthWorld) {
    world.send(Method::GET, "/health", None, None).await;
}

#[when(regex = r#"^the app starts sign-in with redirect "([^"]*)"$"#)]
async fn start_with_redirect(world: &mut AuthWorld, redirect: String) {
    world
        .send(Method::GET, &AuthWorld::start_uri(&redirect), None, None)
        .await;
}

#[when(
    regex = r"^the app starts sign-in, the browser follows the callback and the app exchanges the one-time code$"
)]
async fn full_sign_in(world: &mut AuthWorld) {
    world.sign_in().await;
}

#[when(regex = r"^the app exchanges it again$")]
async fn exchange_again(world: &mut AuthWorld) {
    let code = world.code.clone().expect("a code was exchanged before");
    world.exchange(&code, APP_VERIFIER).await;
}

#[when(regex = r"^the app exchanges a valid code with the wrong verifier$")]
async fn exchange_wrong_verifier(world: &mut AuthWorld) {
    let code = world.obtain_code().await;
    world.exchange(&code, "not-the-verifier").await;
}

#[when(regex = r"^the app calls POST /auth/logout$")]
async fn post_logout(world: &mut AuthWorld) {
    let token = world.token.clone().expect("signed in");
    let reply = world
        .send(Method::POST, "/auth/logout", Some(&token), None)
        .await;
    assert_eq!(reply.status, 204, "{reply:?}");
}

#[when(regex = r"^the same IP calls /auth/discord/start (\d+) times within a minute$")]
async fn start_n_times(world: &mut AuthWorld, times: usize) {
    world.replies.clear();
    for _ in 0..times {
        let reply = world
            .send(Method::GET, &AuthWorld::start_uri(REDIRECT), None, None)
            .await;
        world.replies.push(reply);
    }
}

// ---- Then ----

#[then(regex = r#"^it answers 200 with status "([^"]*)"$"#)]
async fn answers_200_with_status(world: &mut AuthWorld, status: String) {
    assert_eq!(world.last().status, 200);
    assert_eq!(world.last().body["status"], status.as_str());
}

#[then(regex = r#"^the server answers (\d+) with code "([^"]*)"$"#)]
async fn server_answers_with_code(world: &mut AuthWorld, status: u16, code: String) {
    assert_error(world.last(), status, &code);
}

#[then(regex = r"^the app receives a session token$")]
async fn app_receives_token(world: &mut AuthWorld) {
    let token = world.token.as_deref().expect("a token was issued");
    assert!(token.len() >= 43, "token must carry at least 32 bytes");
}

#[then(regex = r#"^GET /me with that token returns the display name "([^"]*)"$"#)]
async fn me_returns_display_name(world: &mut AuthWorld, name: String) {
    let token = world.token.clone().expect("token");
    let reply = world.send(Method::GET, "/me", Some(&token), None).await;
    assert_eq!(reply.status, 200, "{reply:?}");
    assert_eq!(reply.body["display_name"], name.as_str());
}

#[then(regex = r"^GET /me with the same token answers (\d+)$")]
async fn me_with_same_token_answers(world: &mut AuthWorld, status: u16) {
    let token = world.token.clone().expect("token");
    let reply = world.send(Method::GET, "/me", Some(&token), None).await;
    assert_eq!(reply.status, status, "{reply:?}");
}

#[then(regex = r#"^GET /me answers (\d+) with code "([^"]*)"$"#)]
async fn me_answers_with_code(world: &mut AuthWorld, status: u16, code: String) {
    let token = world.token.clone().expect("token");
    let reply = world.send(Method::GET, "/me", Some(&token), None).await;
    assert_error(&reply, status, &code);
}

#[then(regex = r#"^the (\d+)(?:st|nd|rd|th) call answers (\d+) with code "([^"]*)"$"#)]
async fn nth_call_answers(world: &mut AuthWorld, nth: usize, status: u16, code: String) {
    assert_eq!(world.replies.len(), nth);
    for earlier in &world.replies[..nth - 1] {
        assert_eq!(earlier.status, 302, "earlier calls succeed: {earlier:?}");
    }
    assert_error(&world.replies[nth - 1], status, &code);
}

#[then(regex = r"^the sessions table does not contain the token in plain text$")]
async fn sessions_hold_no_plain_token(world: &mut AuthWorld) {
    let token = world.token.clone().expect("token");
    let store = world.harness().await.store.clone();
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT id || '|' || user_id || '|' || token_hash FROM sessions")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(rows.len(), 1, "exactly one session is stored");
    assert!(
        !rows[0].contains(&token),
        "the plain token must not be stored"
    );
    let hash: String = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(hash.len(), 64, "a hex SHA-256 digest is stored");
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    AuthWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/community-auth.feature"
        ))
        .await;
}
