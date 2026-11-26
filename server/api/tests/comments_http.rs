//! HTTP-level tests of the mod comments endpoints (router + `oneshot`, in-memory
//! SQLite, fake clock, no network).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::Router;
use chrono::Duration;
use community_api::testing::{
    auth_service, comments_state, downloads_state, sign_in_user, star_service, FakeClock,
    FakeDiscord,
};
use community_api::{build_router, AppState, RateLimits};
use community_application::comments::CommentRepository;
use community_domain::comments::{Comment, CommentBody, CommentId, ModId};
use community_domain::User;
use community_infrastructure::SqliteStore;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

struct Harness {
    app: Router,
    clock: Arc<FakeClock>,
    store: Arc<SqliteStore>,
    ada: (User, String),
    bob: (User, String),
}

async fn harness() -> Harness {
    let clock = Arc::new(FakeClock::at_epoch_plus(1_700_000_000));
    let store = SqliteStore::connect_in_memory().await.unwrap();
    store.migrate().await.unwrap();
    let store = Arc::new(store);
    let state = AppState {
        clock: clock.clone(),
        health: store.clone(),
        auth: auth_service(
            store.clone(),
            clock.clone(),
            Arc::new(FakeDiscord::new("x")),
        ),
        stars: star_service(store.clone()),
        downloads: downloads_state(store.clone(), clock.clone()),
        limits: RateLimits::new(clock.clone()),
        comments: comments_state(store.clone(), clock.clone()),
        mods: community_api::testing::unused_mod_hosting(store.clone(), clock.clone()),
        trust_proxy_headers: false,
    };
    let ada = sign_in_user(&store, clock.as_ref(), "1001", "Ada").await;
    let bob = sign_in_user(&store, clock.as_ref(), "1002", "Bob").await;
    Harness {
        app: build_router(state),
        clock,
        store,
        ada,
        bob,
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    json: Value,
}

impl Harness {
    async fn raw(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Vec<u8>>,
    ) -> Reply {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request_body = match body {
            Some(bytes) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(bytes)
            }
            None => Body::empty(),
        };
        let mut request = builder.body(request_body).unwrap();
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            json: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        }
    }

    async fn send(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        self.raw(method, uri, token, body.map(|v| v.to_string().into_bytes()))
            .await
    }

    async fn get(&self, uri: &str, token: Option<&str>) -> Reply {
        self.send(Method::GET, uri, token, None).await
    }

    async fn post(&self, mod_id: &str, token: &str, body: &str) -> Reply {
        self.send(
            Method::POST,
            &format!("/mods/{mod_id}/comments"),
            Some(token),
            Some(json!({ "body": body })),
        )
        .await
    }

    /// Stores a comment directly (bypasses the write limit) one second after the last.
    async fn seed(&self, mod_id: &str, author: &User, body: &str) -> String {
        self.clock.advance(Duration::seconds(1));
        let id = format!("seed-{}", uuid_like(body));
        self.store
            .insert(&Comment {
                id: CommentId(id.clone()),
                mod_id: ModId::parse(mod_id).unwrap(),
                user_id: author.id.clone(),
                body: CommentBody::parse(body).unwrap(),
                created_at: community_application::ports::Clock::now(self.clock.as_ref()),
                edited_at: None,
            })
            .await
            .unwrap();
        id
    }
}

fn uuid_like(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02x}")).collect()
}

fn bodies(reply: &Reply) -> Vec<String> {
    reply.json["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["body"].as_str().unwrap().to_owned())
        .collect()
}

fn error_code(reply: &Reply) -> &str {
    reply.json["error"]["code"].as_str().unwrap_or("")
}

#[tokio::test]
async fn reading_without_an_account_lists_comments_oldest_first() {
    let h = harness().await;
    for text in ["first", "second", "third"] {
        h.seed("livery-pack", &h.ada.0, text).await;
    }
    let reply = h.get("/mods/livery-pack/comments", None).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(bodies(&reply), ["first", "second", "third"]);
    assert!(reply.json["next_cursor"].is_null());
    let first = &reply.json["comments"][0];
    assert_eq!(first["author"]["display_name"], "Ada");
    assert!(first["author"]["avatar_url"].is_null());
    assert_eq!(first["own"], false);
    assert!(first["edited_at"].is_null());
    assert!(first["created_at"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn an_unknown_mod_has_no_comments() {
    let h = harness().await;
    let reply = h.get("/mods/never-seen/comments", None).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json, json!({ "comments": [], "next_cursor": null }));
}

#[tokio::test]
async fn a_valid_token_marks_own_comments_and_a_bad_token_is_ignored() {
    let h = harness().await;
    h.seed("mod-a", &h.ada.0, "by ada").await;
    h.seed("mod-a", &h.bob.0, "by bob").await;

    let as_ada = h.get("/mods/mod-a/comments", Some(&h.ada.1)).await;
    assert_eq!(as_ada.json["comments"][0]["own"], true);
    assert_eq!(as_ada.json["comments"][1]["own"], false);

    let bad = h
        .get("/mods/mod-a/comments", Some("not-a-real-token"))
        .await;
    assert_eq!(bad.status, StatusCode::OK);
    assert_eq!(bad.json["comments"][0]["own"], false);
}

#[tokio::test]
async fn posting_appends_at_the_end_and_answers_201() {
    let h = harness().await;
    h.seed("mod-a", &h.bob.0, "older").await;
    h.clock.advance(Duration::seconds(5));

    let reply = h.post("mod-a", &h.ada.1, "Great livery").await;
    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.json["body"], "Great livery");
    assert_eq!(reply.json["own"], true);
    assert_eq!(reply.json["author"]["display_name"], "Ada");

    let list = h.get("/mods/mod-a/comments", None).await;
    assert_eq!(bodies(&list), ["older", "Great livery"]);
}

#[tokio::test]
async fn posting_requires_a_session() {
    let h = harness().await;
    let anonymous = h
        .send(
            Method::POST,
            "/mods/mod-a/comments",
            None,
            Some(json!({ "body": "hi" })),
        )
        .await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
    let bad = h.post("mod-a", "wrong", "hi").await;
    assert_eq!(bad.status, StatusCode::UNAUTHORIZED);
    assert!(bodies(&h.get("/mods/mod-a/comments", None).await).is_empty());
}

#[tokio::test]
async fn pagination_gives_20_then_5_in_posting_order() {
    let h = harness().await;
    for i in 0..25 {
        h.seed("mod-a", &h.ada.0, &format!("c{i:02}")).await;
    }
    let first = h.get("/mods/mod-a/comments", None).await;
    assert_eq!(first.status, StatusCode::OK);
    let first_bodies = bodies(&first);
    assert_eq!(first_bodies.len(), 20);
    assert_eq!(first_bodies[0], "c00");
    assert_eq!(first_bodies[19], "c19");
    let cursor = first.json["next_cursor"].as_str().expect("a next cursor");

    let second = h
        .get(&format!("/mods/mod-a/comments?cursor={cursor}"), None)
        .await;
    assert_eq!(
        bodies(&second),
        ["c20", "c21", "c22", "c23", "c24"].map(str::to_owned)
    );
    assert!(second.json["next_cursor"].is_null());
}

#[tokio::test]
async fn limit_is_honored_and_validated() {
    let h = harness().await;
    for i in 0..3 {
        h.seed("mod-a", &h.ada.0, &format!("c{i}")).await;
    }
    let two = h.get("/mods/mod-a/comments?limit=2", None).await;
    assert_eq!(bodies(&two).len(), 2);
    assert!(two.json["next_cursor"].is_string());

    for query in ["limit=0", "limit=51", "limit=abc", "limit=-1", "cursor=%%%"] {
        let reply = h.get(&format!("/mods/mod-a/comments?{query}"), None).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(error_code(&reply), "bad_request", "{query}");
    }
    let garbage_cursor = h.get("/mods/mod-a/comments?cursor=garbage", None).await;
    assert_eq!(garbage_cursor.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_mod_ids_are_rejected() {
    let h = harness().await;
    let too_long = "a".repeat(101);
    for mod_id in ["Upper", "with_underscore", too_long.as_str()] {
        assert_eq!(
            h.get(&format!("/mods/{mod_id}/comments"), None)
                .await
                .status,
            StatusCode::BAD_REQUEST,
            "GET {mod_id}"
        );
        assert_eq!(
            h.post(mod_id, &h.ada.1, "hi").await.status,
            StatusCode::BAD_REQUEST,
            "POST {mod_id}"
        );
    }
}

#[tokio::test]
async fn editing_own_comment_updates_the_body_and_sets_edited_at() {
    let h = harness().await;
    let created = h.post("mod-a", &h.ada.1, "Nice").await;
    let id = created.json["id"].as_str().unwrap().to_owned();
    h.clock.advance(Duration::minutes(2));

    let reply = h
        .send(
            Method::PATCH,
            &format!("/mods/mod-a/comments/{id}"),
            Some(&h.ada.1),
            Some(json!({ "body": "Nice livery" })),
        )
        .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["body"], "Nice livery");
    assert!(reply.json["edited_at"].is_string());
    assert_eq!(reply.json["created_at"], created.json["created_at"]);

    let list = h.get("/mods/mod-a/comments", None).await;
    assert_eq!(bodies(&list), ["Nice livery"]);
}

#[tokio::test]
async fn someone_elses_comment_cannot_be_edited_or_deleted() {
    let h = harness().await;
    let id = h.seed("mod-a", &h.bob.0, "Bob says").await;
    let uri = format!("/mods/mod-a/comments/{id}");

    let edit = h
        .send(
            Method::PATCH,
            &uri,
            Some(&h.ada.1),
            Some(json!({ "body": "Hijacked" })),
        )
        .await;
    assert_eq!(edit.status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&edit), "not_found");

    let delete = h.send(Method::DELETE, &uri, Some(&h.ada.1), None).await;
    assert_eq!(delete.status, StatusCode::NOT_FOUND);

    let list = h.get("/mods/mod-a/comments", None).await;
    assert_eq!(bodies(&list), ["Bob says"]);
}

#[tokio::test]
async fn unknown_ids_and_other_mods_are_not_found() {
    let h = harness().await;
    let id = h.seed("mod-a", &h.ada.0, "mine").await;
    for uri in [
        format!("/mods/mod-b/comments/{id}"),
        "/mods/mod-a/comments/missing".to_owned(),
    ] {
        let edit = h
            .send(
                Method::PATCH,
                &uri,
                Some(&h.ada.1),
                Some(json!({ "body": "x" })),
            )
            .await;
        assert_eq!(edit.status, StatusCode::NOT_FOUND, "PATCH {uri}");
        let delete = h.send(Method::DELETE, &uri, Some(&h.ada.1), None).await;
        assert_eq!(delete.status, StatusCode::NOT_FOUND, "DELETE {uri}");
    }
}

#[tokio::test]
async fn editing_and_deleting_require_a_session() {
    let h = harness().await;
    let id = h.seed("mod-a", &h.ada.0, "mine").await;
    let uri = format!("/mods/mod-a/comments/{id}");
    let edit = h
        .send(Method::PATCH, &uri, None, Some(json!({ "body": "x" })))
        .await;
    assert_eq!(edit.status, StatusCode::UNAUTHORIZED);
    let delete = h.send(Method::DELETE, &uri, None, None).await;
    assert_eq!(delete.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn deleting_own_comment_removes_it() {
    let h = harness().await;
    let created = h.post("mod-a", &h.ada.1, "Oops").await;
    let id = created.json["id"].as_str().unwrap();
    let uri = format!("/mods/mod-a/comments/{id}");

    let reply = h.send(Method::DELETE, &uri, Some(&h.ada.1), None).await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);
    assert!(bodies(&h.get("/mods/mod-a/comments", None).await).is_empty());

    let again = h.send(Method::DELETE, &uri, Some(&h.ada.1), None).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bodies_over_2000_characters_are_rejected_and_nothing_is_stored() {
    let h = harness().await;
    let reply = h.post("mod-a", &h.ada.1, &"a".repeat(2001)).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&reply), "bad_request");
    assert!(bodies(&h.get("/mods/mod-a/comments", None).await).is_empty());

    let exactly = h.post("mod-a", &h.ada.1, &"a".repeat(2000)).await;
    assert_eq!(exactly.status, StatusCode::CREATED);
}

#[tokio::test]
async fn empty_control_char_and_malformed_bodies_are_bad_requests() {
    let h = harness().await;
    assert_eq!(
        h.post("mod-a", &h.ada.1, "   ").await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.post("mod-a", &h.ada.1, "bad\u{0}byte").await.status,
        StatusCode::BAD_REQUEST
    );
    for payload in [&b"not json"[..], b"{}", br#"{"body": 5}"#] {
        let reply = h
            .raw(
                Method::POST,
                "/mods/mod-a/comments",
                Some(&h.ada.1),
                Some(payload.to_vec()),
            )
            .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(error_code(&reply), "bad_request");
    }
    assert!(bodies(&h.get("/mods/mod-a/comments", None).await).is_empty());
}

#[tokio::test]
async fn request_bodies_over_16_kib_are_rejected() {
    let h = harness().await;
    let huge = json!({ "body": "a".repeat(20 * 1024) })
        .to_string()
        .into_bytes();
    let reply = h
        .raw(
            Method::POST,
            "/mods/mod-a/comments",
            Some(&h.ada.1),
            Some(huge),
        )
        .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(bodies(&h.get("/mods/mod-a/comments", None).await).is_empty());
}

#[tokio::test]
async fn crlf_is_normalized_and_whitespace_trimmed() {
    let h = harness().await;
    let reply = h.post("mod-a", &h.ada.1, "  line one\r\nline two  ").await;
    assert_eq!(reply.json["body"], "line one\nline two");
}

#[tokio::test]
async fn raw_html_is_stored_and_returned_verbatim_as_text() {
    let h = harness().await;
    let html = "<img src=x onerror=alert(1)>";
    let reply = h.post("mod-a", &h.ada.1, html).await;
    assert_eq!(reply.status, StatusCode::CREATED);
    let list = h.get("/mods/mod-a/comments", None).await;
    assert_eq!(bodies(&list), [html]);
    let stored: String = sqlx::query_scalar("SELECT body FROM comments")
        .fetch_one(h.store.pool())
        .await
        .unwrap();
    assert_eq!(stored, html);
}

#[tokio::test]
async fn the_sixth_write_in_a_minute_is_rate_limited_with_retry_after() {
    let h = harness().await;
    for i in 0..5 {
        let reply = h.post("mod-a", &h.ada.1, &format!("c{i}")).await;
        assert_eq!(reply.status, StatusCode::CREATED, "write {i}");
    }
    let sixth = h.post("mod-a", &h.ada.1, "one too many").await;
    assert_eq!(sixth.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(error_code(&sixth), "rate_limited");
    let retry_after: u64 = sixth.headers[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry_after));
    assert_eq!(bodies(&h.get("/mods/mod-a/comments", None).await).len(), 5);

    // Reads, other users and later minutes are unaffected.
    assert_eq!(
        h.get("/mods/mod-a/comments", Some(&h.ada.1)).await.status,
        StatusCode::OK
    );
    assert_eq!(
        h.post("mod-a", &h.bob.1, "bob").await.status,
        StatusCode::CREATED
    );
    h.clock
        .advance(Duration::seconds(i64::try_from(retry_after).unwrap()));
    assert_eq!(
        h.post("mod-a", &h.ada.1, "again").await.status,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn edits_and_deletes_count_towards_the_write_limit() {
    let h = harness().await;
    let id = h.post("mod-a", &h.ada.1, "base").await.json["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let uri = format!("/mods/mod-a/comments/{id}");
    for _ in 0..4 {
        let reply = h
            .send(
                Method::PATCH,
                &uri,
                Some(&h.ada.1),
                Some(json!({ "body": "e" })),
            )
            .await;
        assert_eq!(reply.status, StatusCode::OK);
    }
    let blocked = h.send(Method::DELETE, &uri, Some(&h.ada.1), None).await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.headers.get(header::RETRY_AFTER).is_some());
}
