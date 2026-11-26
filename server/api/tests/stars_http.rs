//! HTTP tests of the stars endpoints, driven through the router with `oneshot`.

mod common;

use axum::http::Method;
use chrono::Duration;
use common::Harness;
use serde_json::json;

const RIDER_A: &str = "111111111111111111";
const RIDER_B: &str = "222222222222222222";

#[tokio::test]
async fn put_star_counts_the_star_and_marks_the_caller() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    let reply = h
        .send(Method::PUT, "/mods/ducati-2025/star", Some(&token))
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        json!({ "id": "ducati-2025", "stars": 1, "starredByMe": true })
    );
}

#[tokio::test]
async fn starring_twice_keeps_one_star() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    h.send(Method::PUT, "/mods/m/star", Some(&token)).await;
    let again = h.send(Method::PUT, "/mods/m/star", Some(&token)).await;
    assert_eq!(again.body["stars"], 1);
}

#[tokio::test]
async fn two_users_add_up_and_unstar_removes_only_the_callers_star() {
    let h = Harness::new().await;
    let a = h.sign_in(RIDER_A, "alice").await;
    let b = h.sign_in(RIDER_B, "bob").await;
    h.send(Method::PUT, "/mods/m/star", Some(&a)).await;
    let second = h.send(Method::PUT, "/mods/m/star", Some(&b)).await;
    assert_eq!(second.body["stars"], 2);

    let removed = h.send(Method::DELETE, "/mods/m/star", Some(&a)).await;
    assert_eq!(removed.status, 200);
    assert_eq!(
        removed.body,
        json!({ "id": "m", "stars": 1, "starredByMe": false })
    );
}

#[tokio::test]
async fn unstarring_a_mod_that_is_not_starred_succeeds() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    let reply = h
        .send(Method::DELETE, "/mods/never/star", Some(&token))
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        json!({ "id": "never", "stars": 0, "starredByMe": false })
    );
}

#[tokio::test]
async fn star_and_unstar_need_a_session() {
    let h = Harness::new().await;
    for method in [Method::PUT, Method::DELETE] {
        let missing = h.send(method.clone(), "/mods/m/star", None).await;
        assert_eq!(missing.status, 401);
        assert_eq!(missing.body["error"]["code"], "unauthorized");
        let bogus = h.send(method, "/mods/m/star", Some("not-a-token")).await;
        assert_eq!(bogus.status, 401);
    }
}

#[tokio::test]
async fn invalid_mod_ids_are_bad_requests() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    for id in [
        "Not%20A%20Slug!",
        "UPPER",
        "-lead",
        "dou--ble",
        "under_score",
    ] {
        let reply = h
            .send(Method::PUT, &format!("/mods/{id}/star"), Some(&token))
            .await;
        assert_eq!(reply.status, 400, "{id}");
        assert_eq!(reply.body["error"]["code"], "bad_request", "{id}");
    }
    let too_long = "a".repeat(101);
    let reply = h
        .send(
            Method::DELETE,
            &format!("/mods/{too_long}/star"),
            Some(&token),
        )
        .await;
    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn stats_are_anonymous_without_a_session() {
    let h = Harness::new().await;
    let a = h.sign_in(RIDER_A, "alice").await;
    let b = h.sign_in(RIDER_B, "bob").await;
    h.send(Method::PUT, "/mods/zeta/star", Some(&a)).await;
    h.send(Method::PUT, "/mods/zeta/star", Some(&b)).await;
    h.send(Method::PUT, "/mods/alpha/star", Some(&b)).await;
    h.send(Method::PUT, "/mods/gone/star", Some(&a)).await;
    h.send(Method::DELETE, "/mods/gone/star", Some(&a)).await;

    let reply = h.send(Method::GET, "/mods/stats", None).await;
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        json!({ "mods": [{ "id": "alpha", "stars": 1 }, { "id": "zeta", "stars": 2 }] })
    );
}

#[tokio::test]
async fn stats_with_a_session_flag_the_callers_stars() {
    let h = Harness::new().await;
    let a = h.sign_in(RIDER_A, "alice").await;
    let b = h.sign_in(RIDER_B, "bob").await;
    h.send(Method::PUT, "/mods/alpha/star", Some(&a)).await;
    h.send(Method::PUT, "/mods/zeta/star", Some(&b)).await;

    let reply = h.send(Method::GET, "/mods/stats", Some(&a)).await;
    assert_eq!(
        reply.body,
        json!({ "mods": [
            { "id": "alpha", "stars": 1, "starredByMe": true },
            { "id": "zeta", "stars": 1, "starredByMe": false }
        ] })
    );
}

#[tokio::test]
async fn stats_are_empty_when_nothing_is_starred() {
    let h = Harness::new().await;
    let reply = h.send(Method::GET, "/mods/stats", None).await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body, json!({ "mods": [] }));
}

#[tokio::test]
async fn stats_answer_anonymously_for_a_stale_session() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    h.send(Method::PUT, "/mods/m/star", Some(&token)).await;

    let unknown = h
        .send(Method::GET, "/mods/stats", Some("not-a-token"))
        .await;
    assert_eq!(unknown.status, 200);
    assert_eq!(unknown.body, json!({ "mods": [{ "id": "m", "stars": 1 }] }));

    h.clock.advance(Duration::days(31));
    let expired = h.send(Method::GET, "/mods/stats", Some(&token)).await;
    assert_eq!(expired.status, 200);
    assert_eq!(expired.body, json!({ "mods": [{ "id": "m", "stars": 1 }] }));
}

#[tokio::test]
async fn stats_answer_anonymously_after_logout() {
    let h = Harness::new().await;
    let token = h.sign_in(RIDER_A, "alice").await;
    h.send(Method::PUT, "/mods/m/star", Some(&token)).await;
    let logout = h.send(Method::POST, "/auth/logout", Some(&token)).await;
    assert_eq!(logout.status, 204);
    let reply = h.send(Method::GET, "/mods/stats", Some(&token)).await;
    assert_eq!(reply.body, json!({ "mods": [{ "id": "m", "stars": 1 }] }));
}

#[tokio::test]
async fn a_valid_session_pays_the_per_user_limit_on_every_star_route() {
    // Signing in does not count: the limit is per user on authenticated routes.
    let h = Harness::with_user_limit(2).await;
    let token = h.sign_in(RIDER_A, "alice").await;
    assert_eq!(
        h.send(Method::GET, "/mods/stats", Some(&token))
            .await
            .status,
        200
    );
    assert_eq!(
        h.send(Method::PUT, "/mods/m/star", Some(&token))
            .await
            .status,
        200
    );
    let limited = h.send(Method::GET, "/mods/stats", Some(&token)).await;
    assert_eq!(limited.status, 429);
    assert_eq!(limited.body["error"]["code"], "rate_limited");

    // Anonymous stats are not subject to the per-user limit.
    assert_eq!(h.send(Method::GET, "/mods/stats", None).await.status, 200);
}
