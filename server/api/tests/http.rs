//! HTTP-level tests of the skeleton, driven through the router with `oneshot`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use chrono::Duration;
use community_api::testing::{auth_service, star_service, FakeClock, FakeDiscord};
use community_api::{build_router, AppState, RateLimits};
use community_application::ports::{HealthProbe, PortResult};
use community_application::AppError;
use community_infrastructure::SqliteStore;
use http_body_util::BodyExt;
use tower::ServiceExt;

struct BrokenDb;

#[async_trait::async_trait]
impl HealthProbe for BrokenDb {
    async fn check(&self) -> PortResult<()> {
        Err(AppError::Internal(
            "database file /var/secret.db is locked".into(),
        ))
    }
}

async fn state_with(health: Arc<dyn HealthProbe>, clock: Arc<FakeClock>, trust: bool) -> AppState {
    let store = SqliteStore::connect_in_memory().await.unwrap();
    store.migrate().await.unwrap();
    let store = Arc::new(store);
    let auth = auth_service(
        store.clone(),
        clock.clone(),
        Arc::new(FakeDiscord::new("rider")),
    );
    AppState {
        clock: clock.clone(),
        health,
        auth,
        stars: star_service(store),
        limits: RateLimits::new(clock),
        trust_proxy_headers: trust,
    }
}

async fn healthy_state(clock: Arc<FakeClock>, trust: bool) -> AppState {
    let store = SqliteStore::connect_in_memory().await.unwrap();
    store.migrate().await.unwrap();
    state_with(Arc::new(store), clock, trust).await
}

fn request(path: &str, ip: &str) -> Request<Body> {
    let mut request = Request::builder().uri(path).body(Body::empty()).unwrap();
    let addr: SocketAddr = format!("{ip}:4000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(addr));
    request
}

async fn call(
    app: &Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, headers, json)
}

fn clock() -> Arc<FakeClock> {
    Arc::new(FakeClock::at_epoch_plus(1_000_000))
}

#[tokio::test]
async fn health_answers_ok_when_the_database_works() {
    let app = build_router(healthy_state(clock(), false).await);
    let (status, _, json) = call(&app, request("/health", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, serde_json::json!({ "status": "ok" }));
}

#[tokio::test]
async fn health_fails_with_a_generic_error_when_the_database_is_down() {
    let app = build_router(state_with(Arc::new(BrokenDb), clock(), false).await);
    let (status, _, json) = call(&app, request("/health", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "internal");
    assert!(!json.to_string().contains("secret.db"));
}

#[tokio::test]
async fn unknown_routes_answer_the_json_not_found_error() {
    let app = build_router(healthy_state(clock(), false).await);
    let (status, _, json) = call(&app, request("/nope", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].is_string());
}

#[tokio::test]
async fn cors_is_closed() {
    let app = build_router(healthy_state(clock(), false).await);

    let mut simple = request("/health", "10.0.0.1");
    simple
        .headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    let (_, headers, _) = call(&app, simple).await;
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

    let mut preflight = request("/health", "10.0.0.1");
    *preflight.method_mut() = axum::http::Method::OPTIONS;
    preflight
        .headers_mut()
        .insert(header::ORIGIN, "https://evil.example".parse().unwrap());
    preflight.headers_mut().insert(
        header::ACCESS_CONTROL_REQUEST_METHOD,
        "GET".parse().unwrap(),
    );
    let (_, headers, _) = call(&app, preflight).await;
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_METHODS).is_none());
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_HEADERS).is_none());
}

#[tokio::test]
async fn auth_paths_allow_10_per_minute_per_ip_then_429_with_retry_after() {
    let app = build_router(healthy_state(clock(), false).await);
    for _ in 0..10 {
        let (status, _, _) = call(&app, request("/auth/discord/start", "10.0.0.1")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST); // missing params, but counted
    }
    let (status, headers, json) = call(&app, request("/auth/discord/start", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(json["error"]["code"], "rate_limited");
    assert!(headers.get(header::RETRY_AFTER).is_some());

    // Another IP is unaffected, and so is the general group for the same IP.
    let (status, _, _) = call(&app, request("/auth/discord/start", "10.0.0.2")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(&app, request("/health", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn general_routes_allow_60_per_minute_per_ip_and_recover() {
    let fake = clock();
    let app = build_router(healthy_state(fake.clone(), false).await);
    for _ in 0..60 {
        let (status, _, _) = call(&app, request("/health", "10.0.0.1")).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _, _) = call(&app, request("/health", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    fake.advance(Duration::seconds(2));
    let (status, _, _) = call(&app, request("/health", "10.0.0.1")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn forwarded_for_is_used_only_when_trusted() {
    for trust in [false, true] {
        let app = build_router(healthy_state(clock(), trust).await);
        let mut last = StatusCode::OK;
        for i in 0..61 {
            let mut req = request("/health", "127.0.0.1");
            req.headers_mut()
                .insert("x-forwarded-for", format!("203.0.113.{i}").parse().unwrap());
            last = call(&app, req).await.0;
        }
        // Untrusted: every call is the same connection IP, so call 61 is limited.
        // Trusted: each call has a distinct client IP, so none is limited.
        let expected = if trust {
            StatusCode::OK
        } else {
            StatusCode::TOO_MANY_REQUESTS
        };
        assert_eq!(last, expected, "trust = {trust}");
    }
}

#[tokio::test]
async fn user_limit_is_120_per_minute_per_user() {
    let state = healthy_state(clock(), false).await;
    let user = community_domain::UserId("u1".into());
    for _ in 0..120 {
        state.check_user_limit(&user).unwrap();
    }
    assert!(matches!(
        state.check_user_limit(&user),
        Err(AppError::RateLimited { .. })
    ));
    assert!(state
        .check_user_limit(&community_domain::UserId("u2".into()))
        .is_ok());
}

fn post_json(path: &str, body: &str) -> Request<Body> {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
    req.extensions_mut().insert(ConnectInfo(addr));
    req
}

#[tokio::test]
async fn auth_endpoints_reject_bad_input_with_the_json_error() {
    let app = build_router(healthy_state(clock(), false).await);

    let (status, _, json) = call(&app, request("/me", "10.0.0.1")).await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (401, Some("unauthorized"))
    );

    let mut bad_scheme = request("/me", "10.0.0.1");
    bad_scheme
        .headers_mut()
        .insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
    let (status, _, _) = call(&app, bad_scheme).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _, json) = call(&app, post_json("/auth/logout", "")).await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (401, Some("unauthorized"))
    );

    let (status, _, json) = call(&app, post_json("/auth/token", "not json")).await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (400, Some("bad_request"))
    );
    let (status, _, json) = call(&app, post_json("/auth/token", r#"{"code":"x"}"#)).await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (400, Some("bad_request"))
    );

    let (status, _, json) = call(
        &app,
        request("/auth/discord/callback?code=c&state=unknown", "10.0.0.1"),
    )
    .await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (400, Some("bad_request"))
    );
    let (status, _, json) = call(&app, request("/auth/discord/callback", "10.0.0.1")).await;
    assert_eq!(
        (status.as_u16(), json["error"]["code"].as_str()),
        (400, Some("bad_request"))
    );
}

#[tokio::test]
async fn callback_access_denied_redirects_to_the_loopback_with_the_app_state() {
    let app = build_router(healthy_state(clock(), false).await);
    let challenge = community_domain::pkce::challenge_for("v");
    let start = format!(
        "/auth/discord/start?redirect=http://127.0.0.1:5000/callback&state=abc&code_challenge={challenge}"
    );
    let mut req = request(&start, "10.0.0.1");
    *req.uri_mut() = start.parse().unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    let state = location
        .split("state=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap();

    let denied = format!("/auth/discord/callback?error=access_denied&state={state}");
    let response = app.oneshot(request(&denied, "10.0.0.1")).await.unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(
        response.headers()[header::LOCATION],
        "http://127.0.0.1:5000/callback?error=access_denied&state=abc"
    );
}
