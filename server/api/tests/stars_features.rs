//! BDD runner (cucumber, `harness = false`) for `features/community-stars.feature`.
//!
//! Every step drives the real router (in-memory SQLite, fake clock, fake
//! Discord). The run exits non-zero when a step fails, is missing or ambiguous.

mod common;

use std::fmt;

use axum::http::Method;
use chrono::Duration;
use common::{Harness, Reply};
use cucumber::{given, then, when, World};

#[derive(World, Default)]
struct StarsWorld {
    harness: Option<Harness>,
    token: Option<String>,
    last: Option<Reply>,
}

impl fmt::Debug for StarsWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StarsWorld")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl StarsWorld {
    async fn harness(&mut self) -> &Harness {
        if self.harness.is_none() {
            self.harness = Some(Harness::new().await);
        }
        self.harness.as_ref().unwrap()
    }

    async fn send(&mut self, method: Method, uri: &str, bearer: Option<String>) {
        let reply = self
            .harness()
            .await
            .send(method, uri, bearer.as_deref())
            .await;
        self.last = Some(reply);
    }

    fn last(&self) -> &Reply {
        self.last.as_ref().expect("a request was made")
    }

    fn token(&self) -> String {
        self.token.clone().expect("the user is signed in")
    }

    fn mod_in_stats(&self, id: &str) -> &serde_json::Value {
        assert_eq!(self.last().status, 200, "{:?}", self.last());
        self.last().body["mods"]
            .as_array()
            .expect("a mods array")
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("{id} is listed in {:?}", self.last()))
    }
}

// ---- Given ----

#[given(regex = r"^the user is signed in$")]
async fn user_is_signed_in(world: &mut StarsWorld) {
    let token = world
        .harness()
        .await
        .sign_in("900000000000000001", "rider")
        .await;
    world.token = Some(token);
}

#[given(regex = r#"^the mod "([^"]*)" has (\d+) stars from other users$"#)]
async fn mod_has_stars(world: &mut StarsWorld, id: String, count: u64) {
    for n in 0..count {
        let harness = world.harness().await;
        let token = harness
            .sign_in(&format!("{}", 800_000_000_000_000_000_u64 + n), "other")
            .await;
        let reply = harness
            .send(Method::PUT, &format!("/mods/{id}/star"), Some(&token))
            .await;
        assert_eq!(reply.status, 200, "{reply:?}");
    }
}

#[given(regex = r#"^the user has starred "([^"]*)"$"#)]
async fn user_has_starred(world: &mut StarsWorld, id: String) {
    let token = world.token();
    let reply = world
        .harness()
        .await
        .send(Method::PUT, &format!("/mods/{id}/star"), Some(&token))
        .await;
    assert_eq!(reply.status, 200, "{reply:?}");
}

#[given(regex = r"^their session expired$")]
async fn session_expired(world: &mut StarsWorld) {
    world.harness().await.clock.advance(Duration::days(31));
}

// ---- When ----

#[when(regex = r#"^they star "([^"]*)"$"#)]
async fn they_star(world: &mut StarsWorld, id: String) {
    let token = world.token();
    world
        .send(Method::PUT, &format!("/mods/{id}/star"), Some(token))
        .await;
}

#[when(regex = r#"^they unstar "([^"]*)"$"#)]
async fn they_unstar(world: &mut StarsWorld, id: String) {
    let token = world.token();
    world
        .send(Method::DELETE, &format!("/mods/{id}/star"), Some(token))
        .await;
}

#[when(regex = r#"^they star the mod id "([^"]*)"$"#)]
async fn they_star_raw_id(world: &mut StarsWorld, id: String) {
    let token = world.token();
    let encoded = id.replace(' ', "%20");
    world
        .send(Method::PUT, &format!("/mods/{encoded}/star"), Some(token))
        .await;
}

#[when(regex = r"^stats are requested without a session$")]
async fn stats_anonymous(world: &mut StarsWorld) {
    world.send(Method::GET, "/mods/stats", None).await;
}

#[when(regex = r"^stats are requested with their session$")]
async fn stats_with_session(world: &mut StarsWorld) {
    let token = world.token();
    world.send(Method::GET, "/mods/stats", Some(token)).await;
}

#[when(regex = r"^a star is sent without a valid session$")]
async fn star_without_session(world: &mut StarsWorld) {
    world
        .send(Method::PUT, "/mods/ducati-2025/star", None)
        .await;
}

// ---- Then ----

#[then(regex = r"^the server answers (\d+) with (\d+) stars and starredByMe (true|false)$")]
async fn answers_with_stars(world: &mut StarsWorld, status: u16, stars: u64, mine: bool) {
    let reply = world.last();
    assert_eq!(reply.status, status, "{reply:?}");
    assert_eq!(reply.body["stars"], stars, "{reply:?}");
    assert_eq!(reply.body["starredByMe"], mine, "{reply:?}");
}

#[then(regex = r#"^the server answers (\d+) with code "([^"]*)"$"#)]
async fn answers_with_code(world: &mut StarsWorld, status: u16, code: String) {
    let reply = world.last();
    assert_eq!(reply.status, status, "{reply:?}");
    assert_eq!(reply.body["error"]["code"], code.as_str(), "{reply:?}");
}

#[then(regex = r#"^"([^"]*)" shows (\d+) stars and no starredByMe flag$"#)]
async fn shows_stars_without_flag(world: &mut StarsWorld, id: String, stars: u64) {
    let entry = world.mod_in_stats(&id);
    assert_eq!(entry["stars"], stars, "{entry}");
    assert!(entry.get("starredByMe").is_none(), "{entry}");
}

#[then(regex = r#"^"([^"]*)" shows (\d+) stars and starredByMe (true|false)$"#)]
async fn shows_stars_with_flag(world: &mut StarsWorld, id: String, stars: u64, mine: bool) {
    let entry = world.mod_in_stats(&id);
    assert_eq!(entry["stars"], stars, "{entry}");
    assert_eq!(entry["starredByMe"], mine, "{entry}");
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    StarsWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/community-stars.feature"
        ))
        .await;
}
