//! BDD runner (cucumber, `harness = false`) for `features/mod-comments.feature`.
//!
//! Every step drives the real router (in-memory SQLite, fake clock). The run
//! exits non-zero when a step fails, is missing or ambiguous.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request};
use axum::Router;
use chrono::Duration;
use community_api::testing::{
    auth_service, comments_state, downloads_state, sign_in_user, star_service, FakeClock,
    FakeDiscord,
};
use community_api::{build_router, AppState, RateLimits};
use community_application::comments::CommentRepository;
use community_application::ports::Clock;
use community_domain::comments::{Comment, CommentBody, CommentId, ModId};
use community_domain::User;
use community_infrastructure::SqliteStore;
use cucumber::{given, then, when, World};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

/// What the last request answered.
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
    /// Display name -> (user, bearer token).
    users: HashMap<String, (User, String)>,
}

#[derive(World, Default)]
struct CommentsWorld {
    harness: Option<Harness>,
    last: Option<Reply>,
    /// The comment the last "posted" step created: `(mod id, comment id)`.
    target: Option<(String, String)>,
    next_cursor: Option<String>,
    /// Bodies seen across the listed pages, in order.
    seen: Vec<String>,
    seeded: u32,
}

impl fmt::Debug for CommentsWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommentsWorld")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl CommentsWorld {
    fn harness(&self) -> &Harness {
        self.harness
            .as_ref()
            .expect("the Background creates the server")
    }

    fn token(&self, name: &str) -> String {
        self.harness().users[name].1.clone()
    }

    async fn send(
        &mut self,
        method: Method,
        uri: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
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
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.harness().app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let reply = Reply {
            status,
            retry_after,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        };
        self.last = Some(reply.clone());
        reply
    }

    async fn post_as(&mut self, name: &str, mod_id: &str, text: &str) -> Reply {
        let token = self.token(name);
        let reply = self
            .send(
                Method::POST,
                &format!("/mods/{mod_id}/comments"),
                Some(&token),
                Some(json!({ "body": text })),
            )
            .await;
        if reply.status == 201 {
            let id = reply.body["id"].as_str().expect("comment id").to_owned();
            self.target = Some((mod_id.to_owned(), id));
        }
        reply
    }

    /// All comment bodies of a mod, read anonymously.
    async fn bodies_of(&mut self, mod_id: &str) -> Vec<String> {
        let reply = self
            .send(
                Method::GET,
                &format!("/mods/{mod_id}/comments?limit=50"),
                None,
                None,
            )
            .await;
        assert_eq!(reply.status, 200, "{reply:?}");
        comment_bodies(&reply)
    }

    fn last(&self) -> &Reply {
        self.last.as_ref().expect("a request was made")
    }

    /// Records a listing: the bodies seen so far and the next cursor.
    fn remember_page(&mut self, reply: &Reply) {
        self.seen.extend(comment_bodies(reply));
        self.next_cursor = reply.body["next_cursor"].as_str().map(str::to_owned);
    }
}

fn comment_bodies(reply: &Reply) -> Vec<String> {
    reply.body["comments"]
        .as_array()
        .expect("a comments array")
        .iter()
        .map(|c| c["body"].as_str().expect("a body").to_owned())
        .collect()
}

// ---- Background / Given ----

#[given(regex = r#"^a community server with the users "([^"]*)" and "([^"]*)"$"#)]
async fn community_server(world: &mut CommentsWorld, first: String, second: String) {
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
        trust_proxy_headers: false,
    };
    let mut users = HashMap::new();
    for (index, name) in [first, second].into_iter().enumerate() {
        let discord_id = (1001 + index).to_string();
        let signed = sign_in_user(&store, clock.as_ref(), &discord_id, &name).await;
        users.insert(name, signed);
    }
    world.harness = Some(Harness {
        app: build_router(state),
        clock,
        store,
        users,
    });
}

#[given(regex = r#"^the mod "([^"]*)" has (\d+) comments$"#)]
async fn mod_has_comments(world: &mut CommentsWorld, mod_id: String, count: u32) {
    let (ada, _) = world.harness().users["Ada"].clone();
    let (bob, _) = world.harness().users["Bob"].clone();
    for index in 0..count {
        let author = if index % 2 == 0 { &ada } else { &bob };
        world.seeded += 1;
        let comment = Comment {
            id: CommentId(format!("seed{}", world.seeded)),
            mod_id: ModId::parse(&mod_id).unwrap(),
            user_id: author.id.clone(),
            body: CommentBody::parse(&format!("comment {index:02}")).unwrap(),
            created_at: world.harness().clock.now(),
            edited_at: None,
        };
        world.harness().store.insert(&comment).await.unwrap();
        // Later comments (seeded or posted) always sort after this one.
        world.harness().clock.advance(Duration::seconds(1));
    }
}

#[given(regex = r#"^"([^"]*)" is signed in$"#)]
async fn is_signed_in(world: &mut CommentsWorld, name: String) {
    let token = world.token(&name);
    let reply = world.send(Method::GET, "/me", Some(&token), None).await;
    assert_eq!(reply.status, 200, "{reply:?}");
    assert_eq!(reply.body["display_name"], name.as_str());
}

#[given(regex = r#"^"([^"]*)" posted "([^"]*)" on "([^"]*)"$"#)]
async fn posted_on(world: &mut CommentsWorld, name: String, text: String, mod_id: String) {
    let reply = world.post_as(&name, &mod_id, &text).await;
    assert_eq!(reply.status, 201, "{reply:?}");
}

#[given(regex = r#"^"([^"]*)" posted (\d+) comments in the last minute$"#)]
async fn posted_n_in_the_last_minute(world: &mut CommentsWorld, name: String, count: u32) {
    for index in 0..count {
        let reply = world
            .post_as(&name, "limit-test", &format!("burst {index}"))
            .await;
        assert_eq!(reply.status, 201, "{reply:?}");
    }
}

// ---- When ----

#[when(regex = r#"^an anonymous client lists the comments of "([^"]*)"$"#)]
async fn anonymous_lists(world: &mut CommentsWorld, mod_id: String) {
    world.seen.clear();
    let reply = world
        .send(Method::GET, &format!("/mods/{mod_id}/comments"), None, None)
        .await;
    assert_eq!(reply.status, 200, "{reply:?}");
    world.remember_page(&reply);
    world.target = Some((mod_id, String::new()));
}

#[when(regex = r"^the client asks for the next page$")]
async fn next_page(world: &mut CommentsWorld) {
    let cursor = world.next_cursor.clone().expect("a cursor was returned");
    let (mod_id, _) = world.target.clone().expect("a mod was listed");
    let reply = world
        .send(
            Method::GET,
            &format!("/mods/{mod_id}/comments?cursor={cursor}"),
            None,
            None,
        )
        .await;
    assert_eq!(reply.status, 200, "{reply:?}");
    world.remember_page(&reply);
}

#[when(regex = r#"^"([^"]*)" posts "([^"]*)" on "([^"]*)"$"#)]
async fn posts_on(world: &mut CommentsWorld, name: String, text: String, mod_id: String) {
    world.post_as(&name, &mod_id, &text).await;
}

#[when(regex = r#"^"([^"]*)" posts a comment of (\d+) characters on "([^"]*)"$"#)]
async fn posts_long(world: &mut CommentsWorld, name: String, length: usize, mod_id: String) {
    world.post_as(&name, &mod_id, &"a".repeat(length)).await;
}

#[when(regex = r#"^"([^"]*)" edits it to "([^"]*)"$"#)]
async fn edits_it(world: &mut CommentsWorld, name: String, text: String) {
    let (mod_id, id) = world.target.clone().expect("a comment was posted");
    let token = world.token(&name);
    world.harness().clock.advance(Duration::minutes(1));
    world
        .send(
            Method::PATCH,
            &format!("/mods/{mod_id}/comments/{id}"),
            Some(&token),
            Some(json!({ "body": text })),
        )
        .await;
}

#[when(regex = r#"^"([^"]*)" deletes it$"#)]
async fn deletes_it(world: &mut CommentsWorld, name: String) {
    let (mod_id, id) = world.target.clone().expect("a comment was posted");
    let token = world.token(&name);
    world
        .send(
            Method::DELETE,
            &format!("/mods/{mod_id}/comments/{id}"),
            Some(&token),
            None,
        )
        .await;
}

// ---- Then ----

#[then(regex = r"^(\d+) comments are returned$")]
async fn n_comments_returned(world: &mut CommentsWorld, count: usize) {
    assert_eq!(comment_bodies(world.last()).len(), count);
}

#[then(regex = r"^every comment has an author, a date and a text$")]
async fn comments_are_complete(world: &mut CommentsWorld) {
    for comment in world.last().body["comments"].as_array().unwrap() {
        assert!(comment["author"]["display_name"].as_str().is_some());
        assert!(comment["created_at"].as_str().is_some());
        assert!(!comment["body"].as_str().unwrap().is_empty());
        assert_eq!(comment["own"], false, "anonymous readers own nothing");
    }
}

#[then(regex = r"^the server answers (\d+)$")]
async fn server_answers(world: &mut CommentsWorld, status: u16) {
    assert_eq!(world.last().status, status, "{:?}", world.last());
}

#[then(regex = r#"^the last comment of "([^"]*)" is "([^"]*)"$"#)]
async fn last_comment_is(world: &mut CommentsWorld, mod_id: String, text: String) {
    let bodies = world.bodies_of(&mod_id).await;
    assert_eq!(bodies.last().map(String::as_str), Some(text.as_str()));
}

#[then(regex = r"^a next cursor is returned$")]
async fn cursor_returned(world: &mut CommentsWorld) {
    assert!(world.next_cursor.is_some(), "{:?}", world.last());
}

#[then(regex = r"^no next cursor is returned$")]
async fn no_cursor_returned(world: &mut CommentsWorld) {
    assert!(world.next_cursor.is_none(), "{:?}", world.last());
}

#[then(regex = r"^all (\d+) comments were seen in posting order$")]
async fn all_seen_in_order(world: &mut CommentsWorld, count: usize) {
    let expected: Vec<String> = (0..count).map(|i| format!("comment {i:02}")).collect();
    assert_eq!(world.seen, expected);
}

#[then(regex = r#"^the comment shows "([^"]*)" and is marked edited$"#)]
async fn shows_and_edited(world: &mut CommentsWorld, text: String) {
    let comment = &world.last().body;
    assert_eq!(comment["body"], text.as_str());
    assert!(comment["edited_at"].is_string(), "{comment}");
}

#[then(regex = r#"^"([^"]*)" has no comments$"#)]
async fn has_no_comments(world: &mut CommentsWorld, mod_id: String) {
    assert!(world.bodies_of(&mod_id).await.is_empty());
}

#[then(regex = r"^the answer has a Retry-After header$")]
async fn has_retry_after(world: &mut CommentsWorld) {
    let value = world.last().retry_after.clone().expect("Retry-After");
    assert!(value.parse::<u64>().unwrap() >= 1);
}

#[then(regex = r#"^the stored body is "([^"]*)"$"#)]
async fn stored_body_is(world: &mut CommentsWorld, text: String) {
    let (_, id) = world.target.clone().expect("a comment was posted");
    let stored: String = sqlx::query_scalar("SELECT body FROM comments WHERE id = ?1")
        .bind(id)
        .fetch_one(world.harness().store.pool())
        .await
        .unwrap();
    assert_eq!(stored, text);
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    CommentsWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/mod-comments.feature"
        ))
        .await;
}
