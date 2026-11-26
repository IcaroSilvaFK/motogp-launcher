//! HTTP tests of the download routes, driven through the router with `oneshot`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use chrono::Duration;
use community_api::testing::{
    auth_service, comments_state, downloads_state, star_service, FakeClock, FakeDiscord,
};
use community_api::{build_router, AppState, RateLimits};
use community_infrastructure::SqliteStore;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const DAY: i64 = 86_400;
/// 2026-11-26 00:00:00 UTC plus a few hours (day number 20_783).
const START: i64 = 20_783 * DAY + 3_600 * 5;
const INSTALL: &str = "aaaaaaaaaaaaaaaaaaaaaa";

struct Setup {
    app: Router,
    clock: Arc<FakeClock>,
    store: Arc<SqliteStore>,
}

async fn setup(trust: bool) -> Setup {
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
        // Generous global limit so only the downloads limiter is exercised.
        limits: RateLimits::with_limits(clock.clone(), 10_000, 10_000, 10_000),
        trust_proxy_headers: trust,
        stars: star_service(store.clone()),
        downloads: downloads_state(store.clone(), clock.clone()),
        comments: comments_state(store.clone(), clock.clone()),
    };
    Setup {
        app: build_router(state),
        clock,
        store,
    }
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    ip: &str,
    body: Option<String>,
    forwarded: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(value) = forwarded {
        builder = builder.header("x-forwarded-for", value);
    }
    let mut request = builder
        .body(body.map(Body::from).unwrap_or_else(Body::empty))
        .unwrap();
    let addr: SocketAddr = format!("{ip}:4000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(addr));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, json)
}

async fn report(app: &Router, mod_id: &str, install: &str, ip: &str) -> (StatusCode, Value) {
    let body = json!({ "install_id": install }).to_string();
    let (status, _, json) = send(
        app,
        Method::POST,
        &format!("/mods/{mod_id}/downloads"),
        ip,
        Some(body),
        None,
    )
    .await;
    (status, json)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let (status, _, json) = send(app, Method::GET, uri, "10.0.0.9", None, None).await;
    (status, json)
}

#[tokio::test]
async fn a_report_answers_204_and_counts_once_per_day() {
    let s = setup(false).await;
    assert_eq!(
        report(&s.app, "mod-a", INSTALL, "10.0.0.1").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        report(&s.app, "mod-a", INSTALL, "10.0.0.1").await.0,
        StatusCode::NO_CONTENT
    );
    let (status, json) = get(&s.app, "/mods/downloads").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json,
        json!({"mods":[{"id":"mod-a","total":1,"last_30_days":1}]})
    );
    s.clock.advance(Duration::days(1));
    report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
    let (_, json) = get(&s.app, "/mods/downloads").await;
    assert_eq!(json["mods"][0]["total"], 2);
}

#[tokio::test]
async fn totals_list_only_mods_with_downloads_sorted_by_id() {
    let s = setup(false).await;
    assert_eq!(get(&s.app, "/mods/downloads").await.1, json!({"mods":[]}));
    report(&s.app, "zeta", INSTALL, "10.0.0.1").await;
    report(&s.app, "alpha", INSTALL, "10.0.0.1").await;
    let (_, json) = get(&s.app, "/mods/downloads").await;
    let ids: Vec<&str> = json["mods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["alpha", "zeta"]);
}

#[tokio::test]
async fn last_30_days_excludes_older_downloads() {
    let s = setup(false).await;
    report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
    s.clock.advance(Duration::days(30));
    let (_, json) = get(&s.app, "/mods/downloads").await;
    assert_eq!(json["mods"][0]["total"], 1);
    assert_eq!(json["mods"][0]["last_30_days"], 0);
}

#[tokio::test]
async fn daily_has_30_zero_filled_days_ending_today() {
    let s = setup(false).await;
    report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
    s.clock.advance(Duration::days(2));
    report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
    let (status, json) = get(&s.app, "/mods/mod-a/downloads/daily").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "mod-a");
    let days = json["days"].as_array().unwrap();
    assert_eq!(days.len(), 30);
    assert_eq!(days[29]["date"], "2026-11-28");
    assert_eq!(days[29]["count"], 1);
    assert_eq!(days[27]["date"], "2026-11-26");
    assert_eq!(days[27]["count"], 1);
    assert_eq!(days[28]["count"], 0);
    assert_eq!(days[0]["date"], "2026-10-30");
}

#[tokio::test]
async fn an_unknown_mod_has_30_zero_days() {
    let s = setup(false).await;
    let (status, json) = get(&s.app, "/mods/nobody/downloads/daily").await;
    assert_eq!(status, StatusCode::OK);
    let days = json["days"].as_array().unwrap();
    assert_eq!(days.len(), 30);
    assert!(days.iter().all(|d| d["count"] == 0));
}

#[tokio::test]
async fn invalid_ids_and_bodies_are_400_bad_request() {
    let s = setup(false).await;
    let (status, json) = report(&s.app, "Bad_Mod", INSTALL, "10.0.0.1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "bad_request");
    let (status, _) = report(&s.app, "mod-a", "too-short", "10.0.0.1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    for body in ["not json", "{}", r#"{"install_id":5}"#] {
        let (status, _, json) = send(
            &s.app,
            Method::POST,
            "/mods/mod-a/downloads",
            "10.0.0.1",
            Some(body.to_owned()),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(json["error"]["code"], "bad_request");
    }
    let (status, _) = get(&s.app, "/mods/Bad_Mod/downloads/daily").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(get(&s.app, "/mods/downloads").await.1["mods"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn reports_are_limited_to_20_a_minute_per_ip() {
    let s = setup(false).await;
    for i in 0..20 {
        let (status, _) = report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
        assert_eq!(status, StatusCode::NO_CONTENT, "report {i}");
    }
    let body = json!({ "install_id": INSTALL }).to_string();
    let (status, headers, json) = send(
        &s.app,
        Method::POST,
        "/mods/mod-a/downloads",
        "10.0.0.1",
        Some(body),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"]["code"], "rate_limited");
    assert!(headers.contains_key(header::RETRY_AFTER));
    // Another IP is unaffected and reads are not limited by this limiter.
    assert_eq!(
        report(&s.app, "mod-a", INSTALL, "10.0.0.2").await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get(&s.app, "/mods/downloads").await.0, StatusCode::OK);
}

#[tokio::test]
async fn the_limiter_uses_the_forwarded_address_when_trusted() {
    let s = setup(true).await;
    let body = json!({ "install_id": INSTALL }).to_string();
    for _ in 0..20 {
        send(
            &s.app,
            Method::POST,
            "/mods/mod-a/downloads",
            "127.0.0.1",
            Some(body.clone()),
            Some("203.0.113.1"),
        )
        .await;
    }
    let (status, _, _) = send(
        &s.app,
        Method::POST,
        "/mods/mod-a/downloads",
        "127.0.0.1",
        Some(body.clone()),
        Some("203.0.113.1"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _, _) = send(
        &s.app,
        Method::POST,
        "/mods/mod-a/downloads",
        "127.0.0.1",
        Some(body),
        Some("203.0.113.2"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn the_database_never_holds_the_raw_install_id_or_the_ip() {
    let s = setup(false).await;
    report(&s.app, "mod-a", INSTALL, "10.0.0.1").await;
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT mod_id, install_hash, day FROM mod_downloads")
            .fetch_all(s.store.pool())
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.len(), 64);
    assert!(!rows[0].1.contains(INSTALL));
    assert!(!format!("{rows:?}").contains("10.0.0.1"));
}
