//! BDD runner (cucumber, `harness = false`) for `features/downloads.feature`.
//!
//! Every step drives the real router over in-memory SQLite with a fake clock.

use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request};
use axum::Router;
use chrono::Duration;
use community_api::testing::{
    auth_service, comments_state, downloads_state, star_service, FakeClock, FakeDiscord,
};
use community_api::{build_router, AppState, RateLimits};
use community_infrastructure::SqliteStore;
use cucumber::{given, then, when, World};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const MOD: &str = "red-livery";
const INSTALL: &str = "Zm9vYmFyYmF6cXV4MTIzNDU2Nzg5MDEyMzQ";
const IP: &str = "10.0.0.1";
/// 2026-11-26 03:00 UTC.
const START: i64 = 20_783 * 86_400 + 3 * 3_600;

#[derive(Debug, Clone)]
struct Reply {
    status: u16,
    retry_after: Option<String>,
    body: Value,
}

struct Harness {
    app: Router,
    clock: Arc<FakeClock>,
    store: Arc<SqliteStore>,
}

#[derive(World, Default)]
struct DownloadsWorld {
    harness: Option<Harness>,
    replies: Vec<Reply>,
}

impl fmt::Debug for DownloadsWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DownloadsWorld")
            .field("replies", &self.replies)
            .finish_non_exhaustive()
    }
}

impl DownloadsWorld {
    async fn harness(&mut self) -> &Harness {
        if self.harness.is_none() {
            let clock = Arc::new(FakeClock::at_epoch_plus(START));
            let store = SqliteStore::connect_in_memory().await.unwrap();
            store.migrate().await.unwrap();
            let store = Arc::new(store);
            let state = AppState {
                clock: clock.clone(),
                health: store.clone(),
                auth: auth_service(
                    store.clone(),
                    clock.clone(),
                    Arc::new(FakeDiscord::new("rider")),
                ),
                // Generous global limit: only the downloads limiter is under test.
                limits: RateLimits::with_limits(clock.clone(), 10_000, 10_000, 10_000),
                trust_proxy_headers: false,
                stars: star_service(store.clone()),
                downloads: downloads_state(store.clone(), clock.clone()),
                comments: comments_state(store.clone(), clock.clone()),
            };
            self.harness = Some(Harness {
                app: build_router(state),
                clock,
                store,
            });
        }
        self.harness.as_ref().unwrap()
    }

    async fn send(&mut self, method: Method, uri: &str, body: Option<Value>) -> Reply {
        let app = self.harness().await.app.clone();
        let mut builder = Request::builder().method(method).uri(uri);
        let request_body = match body {
            Some(value) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let mut request = builder.body(request_body).unwrap();
        let addr: SocketAddr = format!("{IP}:4000").parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = app.oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let reply = Reply {
            status,
            retry_after,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        };
        self.replies.push(reply.clone());
        reply
    }

    async fn report(&mut self, mod_id: &str, install: &str) -> Reply {
        self.send(
            Method::POST,
            &format!("/mods/{mod_id}/downloads"),
            Some(json!({ "install_id": install })),
        )
        .await
    }

    async fn total_of(&mut self, mod_id: &str) -> (u64, u64) {
        let reply = self.send(Method::GET, "/mods/downloads", None).await;
        let entry = reply.body["mods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == mod_id)
            .expect("the mod is listed")
            .clone();
        (
            entry["total"].as_u64().unwrap(),
            entry["last_30_days"].as_u64().unwrap(),
        )
    }

    fn last(&self) -> &Reply {
        self.replies.last().expect("a request was sent")
    }
}

#[given(regex = r"^a mod has (\d+) downloads$")]
async fn mod_has_downloads(world: &mut DownloadsWorld, count: u32) {
    for n in 0..count {
        // Distinct installs; the rate limiter is out of the way by advancing time.
        let install = format!("{:0>22}", format!("seed{n}"));
        world.report(MOD, &install).await;
        world.harness().await.clock.advance(Duration::seconds(10));
    }
    assert_eq!(world.total_of(MOD).await.0, u64::from(count));
}

#[when("the same install reports it twice on the same day")]
async fn same_install_twice(world: &mut DownloadsWorld) {
    for _ in 0..2 {
        let reply = world.report(MOD, INSTALL).await;
        assert_eq!(reply.status, 204);
    }
}

#[then(regex = r"^the count becomes (\d+)$")]
async fn count_becomes(world: &mut DownloadsWorld, expected: u64) {
    assert_eq!(world.total_of(MOD).await.0, expected);
}

#[given("an install reported a mod yesterday")]
async fn reported_yesterday(world: &mut DownloadsWorld) {
    assert_eq!(world.report(MOD, INSTALL).await.status, 204);
    world.harness().await.clock.advance(Duration::days(1));
}

#[when("it reports the mod today")]
async fn reports_today(world: &mut DownloadsWorld) {
    assert_eq!(world.report(MOD, INSTALL).await.status, 204);
}

#[then(regex = r"^the mod has (\d+) downloads and the last 30 days show (\d+)$")]
async fn total_and_recent(world: &mut DownloadsWorld, total: u64, recent: u64) {
    assert_eq!(world.total_of(MOD).await, (total, recent));
}

#[when("an install reports a mod")]
async fn install_reports(world: &mut DownloadsWorld) {
    assert_eq!(world.report(MOD, INSTALL).await.status, 204);
}

#[then("the database holds the SHA-256 of the install id and never the raw id")]
async fn only_the_hash(world: &mut DownloadsWorld) {
    let store = world.harness().await.store.clone();
    let hashes: Vec<String> = sqlx::query_scalar("SELECT install_hash FROM mod_downloads")
        .fetch_all(store.pool())
        .await
        .unwrap();
    let expected: String = Sha256::digest(INSTALL.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(hashes, vec![expected]);
    assert!(!hashes[0].contains(INSTALL));
}

#[when("a report has a malformed mod id or install id")]
async fn malformed_reports(world: &mut DownloadsWorld) {
    world.report("Bad_Mod", INSTALL).await;
    world.report(MOD, "short").await;
}

#[then("the server answers 400 bad_request in the JSON error format")]
async fn bad_request(world: &mut DownloadsWorld) {
    let replies = world.replies.clone();
    assert_eq!(replies.len(), 2);
    for reply in replies {
        assert_eq!(reply.status, 400);
        assert_eq!(reply.body["error"]["code"], "bad_request");
        assert!(reply.body["error"]["message"].is_string());
    }
}

#[when("one IP sends more than 20 reports in a minute")]
async fn many_reports(world: &mut DownloadsWorld) {
    for _ in 0..21 {
        world.report(MOD, INSTALL).await;
    }
}

#[then("the server answers 429 rate_limited with Retry-After")]
async fn rate_limited(world: &mut DownloadsWorld) {
    let last = world.last();
    assert_eq!(last.status, 429);
    assert_eq!(last.body["error"]["code"], "rate_limited");
    assert!(last.retry_after.is_some());
}

#[when("the daily downloads of the mod are requested")]
async fn daily_requested(world: &mut DownloadsWorld) {
    world
        .send(Method::GET, &format!("/mods/{MOD}/downloads/daily"), None)
        .await;
}

#[then("there are 30 UTC days, oldest first, ending today, with zero for days without downloads")]
async fn thirty_buckets(world: &mut DownloadsWorld) {
    let reply = world.last().clone();
    assert_eq!(reply.status, 200);
    let days = reply.body["days"].as_array().unwrap();
    assert_eq!(days.len(), 30);
    let dates: Vec<&str> = days.iter().map(|d| d["date"].as_str().unwrap()).collect();
    let mut sorted = dates.clone();
    sorted.sort_unstable();
    assert_eq!(dates, sorted, "oldest first");
    assert_eq!(dates[29], "2026-11-27", "the last bucket is today (UTC)");
    // Reported on 2026-11-26 (yesterday), nothing today.
    assert_eq!(days[28]["count"], 1);
    assert_eq!(days.iter().filter(|d| d["count"] != 0).count(), 1);
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    DownloadsWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/downloads.feature"
        ))
        .await;
}
