//! BDD runner (cucumber, `harness = false`) for `features/dev-mode.feature`.
//!
//! Drives the real router (in-memory SQLite, fake clock) with the dev-mode
//! adapter and seeder; no network is involved.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request};
use axum::Router;
use community_api::testing::{
    auth_service, comments_state, dev_seeder, downloads_state, mod_hosting, moderation_service,
    star_service, FakeClock,
};
use community_api::{build_router, AppState, RateLimits};
use community_application::dev_seed::{DevSeeder, SeedOutcome};
use community_application::mods::ModLimits;
use community_domain::{pkce, DiscordId};
use community_infrastructure::config::DEFAULT_DEV_FAKE_DISCORD_ID;
use community_infrastructure::{Config, ConfigError, DevFakeDiscord, SqliteStore};
use cucumber::{given, then, when, World};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const BASE_URL: &str = "http://127.0.0.1:8787";
const REDIRECT: &str = "http://127.0.0.1:54321/callback";
const APP_STATE: &str = "app-state-123";
const APP_VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

struct Harness {
    app: Router,
    seeder: DevSeeder,
    _storage: tempfile::TempDir,
}

#[derive(World, Default)]
struct DevWorld {
    harness: Option<Harness>,
    session: Option<Value>,
    startup: Option<Result<Config, ConfigError>>,
    seed_outcomes: Vec<SeedOutcome>,
}

impl fmt::Debug for DevWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DevWorld")
            .field("session", &self.session)
            .field("seed_outcomes", &self.seed_outcomes)
            .finish_non_exhaustive()
    }
}

async fn harness() -> Harness {
    let clock = Arc::new(FakeClock::at_epoch_plus(1_700_000_000));
    let store = SqliteStore::connect_in_memory().await.unwrap();
    store.migrate().await.unwrap();
    let store = Arc::new(store);
    let storage = tempfile::tempdir().unwrap();
    let admins = vec![DiscordId::parse(DEFAULT_DEV_FAKE_DISCORD_ID).unwrap()];
    let hosting = mod_hosting(
        store.clone(),
        clock.clone(),
        storage.path(),
        admins.clone(),
        ModLimits {
            max_upload_bytes: 1_000_000,
            user_quota_bytes: 10_000_000,
            uploads_per_day: 10,
        },
    );
    let seeder = dev_seeder(
        store.clone(),
        clock.clone(),
        hosting.service.clone(),
        admins.clone(),
    );
    let discord = Arc::new(DevFakeDiscord::new(
        BASE_URL,
        "dev-user",
        DEFAULT_DEV_FAKE_DISCORD_ID,
    ));
    let state = AppState {
        clock: clock.clone(),
        health: store.clone(),
        auth: auth_service(store.clone(), clock.clone(), discord),
        stars: star_service(store.clone()),
        downloads: downloads_state(store.clone(), clock.clone()),
        mods: hosting,
        limits: RateLimits::with_limits(clock.clone(), 10_000, 10_000, 10_000),
        moderation: moderation_service(store.clone(), clock.clone(), admins),
        comments: comments_state(store, clock),
        trust_proxy_headers: false,
    };
    Harness {
        app: build_router(state),
        seeder,
        _storage: storage,
    }
}

impl Harness {
    async fn request(&self, request: Request<Body>) -> (u16, Option<String>, Value) {
        let mut request = request;
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            location,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// GET `uri` (a path, or an absolute URL on the server) and return the body.
    async fn get(&self, uri: &str, bearer: Option<&str>) -> Value {
        let (status, _, body) = self.get_raw(uri, bearer).await;
        assert_eq!(status, 200, "{uri}: {body}");
        body
    }

    async fn get_raw(&self, uri: &str, bearer: Option<&str>) -> (u16, Option<String>, Value) {
        let path = uri.strip_prefix(BASE_URL).unwrap_or(uri);
        let mut builder = Request::builder().uri(path);
        if let Some(token) = bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.request(builder.body(Body::empty()).unwrap()).await
    }
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_owned())
    })
}

#[given(regex = r"^the server runs with DEV_FAKE_AUTH on a loopback base URL$")]
async fn dev_server(world: &mut DevWorld) {
    world.harness = Some(harness().await);
}

#[when(regex = r"^the launcher starts a Discord sign-in$")]
async fn launcher_signs_in(world: &mut DevWorld) {
    let harness = world.harness.as_ref().expect("the server runs");
    let start = format!(
        "/auth/discord/start?redirect={REDIRECT}&state={APP_STATE}&code_challenge={}",
        pkce::challenge_for(APP_VERIFIER)
    );
    let (status, authorize, _) = harness.get_raw(&start, None).await;
    assert_eq!(status, 302);
    let authorize = authorize.expect("a redirect");
    // The "consent screen" is the server itself: Discord is never contacted.
    assert!(
        authorize.starts_with(&format!("{BASE_URL}/auth/discord/callback?")),
        "{authorize}"
    );

    let (status, loopback, _) = harness.get_raw(&authorize, None).await;
    assert_eq!(status, 302);
    let loopback = loopback.expect("the loopback redirect");
    assert!(loopback.starts_with(REDIRECT), "{loopback}");
    let code = query_param(&loopback, "code").expect("one-time code");

    let request = Request::builder()
        .method(Method::POST)
        .uri("/auth/token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "code": code, "code_verifier": APP_VERIFIER }).to_string(),
        ))
        .unwrap();
    let (status, _, body) = harness.request(request).await;
    assert_eq!(status, 200, "{body}");
    world.session = Some(body);
}

#[then(regex = r"^it receives a session for the fake user without contacting Discord$")]
async fn receives_session(world: &mut DevWorld) {
    let session = world.session.as_ref().expect("a session was issued");
    let token = session["token"].as_str().expect("a token");
    assert_eq!(session["user"]["username"], "dev-user");
    let me = world
        .harness
        .as_ref()
        .unwrap()
        .get("/me", Some(token))
        .await;
    assert_eq!(me["discord_id"], DEFAULT_DEV_FAKE_DISCORD_ID);
    assert_eq!(me["display_name"], "dev-user");
}

#[given(regex = r"^DEV_FAKE_AUTH is set and PUBLIC_BASE_URL is (https://community\.example\.com)$")]
async fn dev_with_public_url(world: &mut DevWorld, url: String) {
    let vars: HashMap<&str, String> = HashMap::from([
        ("DEV_FAKE_AUTH", "1".to_owned()),
        ("PUBLIC_BASE_URL", url),
        ("DATABASE_URL", "sqlite::memory:".to_owned()),
        ("ADMIN_DISCORD_IDS", "111".to_owned()),
        ("MOD_STORAGE_DIR", "mods".to_owned()),
    ]);
    world.startup = Some(Config::from_lookup(|name| vars.get(name).cloned()));
}

#[then(regex = r"^the server refuses to start$")]
async fn refuses_to_start(world: &mut DevWorld) {
    let startup = world.startup.as_ref().expect("a startup was attempted");
    assert_eq!(
        startup.as_ref().unwrap_err(),
        &ConfigError::DevModeNeedsLoopback("DEV_FAKE_AUTH")
    );
}

#[when(regex = r"^the server starts with seeding twice$")]
async fn seeds_twice(world: &mut DevWorld) {
    let harness = harness().await;
    for _ in 0..2 {
        let outcome = harness.seeder.seed().await.expect("seeding succeeds");
        world.seed_outcomes.push(outcome);
    }
    world.harness = Some(harness);
}

#[then(regex = r"^the demo data exists once$")]
async fn demo_data_once(world: &mut DevWorld) {
    assert_eq!(
        world.seed_outcomes,
        [SeedOutcome::Seeded, SeedOutcome::AlreadySeeded]
    );
    let harness = world.harness.as_ref().unwrap();

    let catalog = harness.get("/catalog", None).await;
    let entries = catalog["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{catalog}");
    assert_eq!(entries[0]["id"], "demo-livery");

    let comments = harness.get("/mods/demo-livery/comments", None).await;
    assert_eq!(
        comments["comments"].as_array().unwrap().len(),
        3,
        "{comments}"
    );

    let stats = harness.get("/mods/stats", None).await;
    let mods = stats["mods"].as_array().unwrap();
    assert_eq!(mods.len(), 1, "{stats}");
    assert_eq!(mods[0]["stars"], 3, "{stats}");

    let downloads = harness.get("/mods/downloads", None).await;
    let rows = downloads["mods"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{downloads}");
    assert_eq!(rows[0]["total"], 5, "{downloads}");
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    DevWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/dev-mode.feature"
        ))
        .await;
}
