//! BDD runner (cucumber, `harness = false`) for `features/mod-hosting.feature`.
//!
//! Every step drives the real router (in-memory SQLite, temp storage dir, fake
//! clock). The run exits non-zero when a step fails, is missing or ambiguous.

mod mod_common;

use std::fmt;

use axum::http::Method;
use cucumber::{given, then, when, World};
use mod_common::{limits, Env, Reply};
use serde_json::{json, Value};

const GIB: u64 = 1024 * 1024 * 1024;

#[derive(World, Default)]
struct ModsWorld {
    env: Option<Env>,
    creator: Option<String>,
    outsider: Option<String>,
    admin: Option<String>,
    last: Option<Reply>,
    upload_id: Option<String>,
    content: Vec<u8>,
    cap: Option<u64>,
}

impl fmt::Debug for ModsWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModsWorld").finish_non_exhaustive()
    }
}

impl ModsWorld {
    async fn env(&mut self) -> &Env {
        if self.env.is_none() {
            let cap = self.cap.unwrap_or(2 * GIB);
            self.env = Some(Env::new(limits(cap, 10 * GIB, 50)).await);
        }
        self.env.as_ref().unwrap()
    }

    async fn creator_token(&mut self) -> String {
        if self.creator.is_none() {
            let token = self.env().await.user("creator", "111").await;
            self.creator = Some(token);
        }
        self.creator.clone().unwrap()
    }

    async fn admin_token(&mut self) -> String {
        if self.admin.is_none() {
            let token = self.env().await.admin().await;
            self.admin = Some(token);
        }
        self.admin.clone().unwrap()
    }

    fn last(&self) -> &Reply {
        self.last.as_ref().expect("a request was made")
    }

    async fn publish(&mut self, mod_id: &str, version: &str, content: &[u8]) {
        let token = self.creator_token().await;
        self.env()
            .await
            .publish(&token, mod_id, version, content, 100)
            .await;
    }

    async fn approve(&mut self, mod_id: &str, version: &str) {
        let admin = self.admin_token().await;
        let reply = self.env().await.approve(&admin, mod_id, version).await;
        assert_eq!(reply.status, 200, "{:?}", reply.json());
    }

    async fn catalog(&mut self) -> Value {
        self.env().await.get("/catalog", None).await.json()
    }

    async fn mine(&mut self) -> Value {
        let token = self.creator_token().await;
        self.env()
            .await
            .get("/mods/mine", Some(&token))
            .await
            .json()
    }
}

fn file_uri(mod_id: &str, version: &str) -> String {
    format!("/files/{mod_id}/{version}/brazil-livery.zip")
}

// ---- Given ----

#[given(regex = r"^the creator is signed in$")]
async fn creator_signed_in(world: &mut ModsWorld) {
    world.creator_token().await;
}

#[given(
    regex = r#"^the creator uploaded "([^"]*)" version "([^"]*)" and it is waiting for review$"#
)]
async fn uploaded_waiting(world: &mut ModsWorld, mod_id: String, version: String) {
    world.publish(&mod_id, &version, b"zip bytes").await;
}

#[given(regex = r"^the size cap is 2 GiB$")]
async fn size_cap(world: &mut ModsWorld) {
    world.cap = Some(2 * GIB);
    world.env().await;
}

#[given(regex = r"^half of a file was uploaded$")]
async fn half_uploaded(world: &mut ModsWorld) {
    world.content = (0..200_u32).map(|n| (n % 251) as u8).collect();
    let token = world.creator_token().await;
    let size = world.content.len() as u64;
    let env = world.env().await;
    let started = env.start(&token, "brazil-livery", "1.0", size).await;
    assert_eq!(started.status, 201);
    let id = started.json()["uploadId"].as_str().unwrap().to_owned();
    let half = world.content[..100].to_vec();
    let reply = world.env().await.put_chunk(&token, &id, 0, &half).await;
    assert_eq!(reply.status, 200);
    world.upload_id = Some(id);
}

#[given(regex = r"^a signed-in user who is not an admin$")]
async fn not_an_admin(world: &mut ModsWorld) {
    let token = world.env().await.user("outsider", "222").await;
    world.outsider = Some(token);
}

#[given(regex = r#"^an admin approved "([^"]*)" version "([^"]*)"$"#)]
async fn admin_approved(world: &mut ModsWorld, mod_id: String, version: String) {
    world.publish(&mod_id, &version, b"zip bytes").await;
    world.approve(&mod_id, &version).await;
}

#[given(regex = r#"^an admin approved a 250-byte file for "([^"]*)" version "([^"]*)"$"#)]
async fn admin_approved_250(world: &mut ModsWorld, mod_id: String, version: String) {
    world.content = (0..250_u32).map(|n| (n % 251) as u8).collect();
    let content = world.content.clone();
    world.publish(&mod_id, &version, &content).await;
    world.approve(&mod_id, &version).await;
}

// ---- When ----

#[when(regex = r#"^they upload "([^"]*)" version "([^"]*)" with a zip file$"#)]
async fn upload_zip(world: &mut ModsWorld, mod_id: String, version: String) {
    world.publish(&mod_id, &version, b"PK zip bytes").await;
}

#[when(regex = r"^the creator declares a 3 GiB upload$")]
async fn declare_3_gib(world: &mut ModsWorld) {
    let token = world.creator_token().await;
    let reply = world
        .env()
        .await
        .start(&token, "brazil-livery", "1.0", 3 * GIB)
        .await;
    world.last = Some(reply);
}

#[when(regex = r"^the connection drops and the creator asks where to continue$")]
async fn ask_where_to_continue(world: &mut ModsWorld) {
    let token = world.creator_token().await;
    let id = world.upload_id.clone().expect("an upload was started");
    let reply = world
        .env()
        .await
        .get(&format!("/mods/uploads/{id}"), Some(&token))
        .await;
    world.last = Some(reply);
}

#[when(regex = r#"^they try to approve "([^"]*)" version "([^"]*)"$"#)]
async fn try_to_approve(world: &mut ModsWorld, mod_id: String, version: String) {
    let token = world.outsider.clone().expect("a non-admin user");
    let reply = world.env().await.approve(&token, &mod_id, &version).await;
    world.last = Some(reply);
}

#[when(regex = r#"^the creator publishes version "([^"]*)" and an admin approves it$"#)]
async fn publish_and_approve(world: &mut ModsWorld, version: String) {
    world.env().await.advance(chrono::Duration::hours(1));
    world
        .publish("brazil-livery", &version, b"newer bytes")
        .await;
    world.approve("brazil-livery", &version).await;
}

#[when(regex = r"^a client asks for bytes 0-99 of the file$")]
async fn ask_range(world: &mut ModsWorld) {
    let reply = world
        .env()
        .await
        .send(
            Method::GET,
            &file_uri("brazil-livery", "1.0"),
            None,
            &[("range", "bytes=0-99")],
            Vec::new(),
            false,
        )
        .await;
    world.last = Some(reply);
}

// ---- Then ----

#[then(regex = r#"^"My mods" lists "([^"]*)" version "([^"]*)" as "([^"]*)"$"#)]
async fn my_mods_lists(world: &mut ModsWorld, mod_id: String, version: String, status: String) {
    let mine = world.mine().await;
    let found = mine["mods"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["id"] == mod_id.as_str())
        .flat_map(|m| m["versions"].as_array().unwrap().iter())
        .any(|v| v["version"] == version.as_str() && v["status"] == status.as_str());
    assert!(found, "{mine}");
}

#[then(regex = r#"^the public catalog does not list "([^"]*)"$"#)]
async fn catalog_lacks(world: &mut ModsWorld, mod_id: String) {
    let catalog = world.catalog().await;
    assert!(catalog["entries"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["id"] != mod_id.as_str()));
}

#[then(regex = r#"^the file of "([^"]*)" version "([^"]*)" cannot be downloaded$"#)]
async fn file_not_downloadable(world: &mut ModsWorld, mod_id: String, version: String) {
    let reply = world
        .env()
        .await
        .get(&file_uri(&mod_id, &version), None)
        .await;
    assert_eq!(reply.status, 404);
}

#[then(regex = r#"^the server answers (\d+) with code "([^"]*)"$"#)]
async fn answers_with_code(world: &mut ModsWorld, status: u16, code: String) {
    assert_eq!(world.last().status, status, "{:?}", world.last().json());
    assert_eq!(world.last().error_code(), code);
}

#[then(regex = r"^no byte was stored$")]
async fn nothing_stored(world: &mut ModsWorld) {
    let dir = world.env().await.dir.path().to_path_buf();
    for area in ["incoming", "pending", "public"] {
        assert_eq!(
            std::fs::read_dir(dir.join(area)).unwrap().count(),
            0,
            "{area}"
        );
    }
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mod_uploads")
        .fetch_one(world.env().await.store.pool())
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[then(regex = r"^the server reports the bytes it already has$")]
async fn reports_received(world: &mut ModsWorld) {
    let json = world.last().json();
    assert_eq!(json["received"], 100);
    assert_eq!(json["size"], 200);
    assert_eq!(json["state"], "uploading");
}

#[then(regex = r"^the upload completes after sending only the rest$")]
async fn completes_with_the_rest(world: &mut ModsWorld) {
    let token = world.creator_token().await;
    let id = world.upload_id.clone().unwrap();
    let rest = world.content[100..].to_vec();
    let env = world.env().await;
    let sent = env.put_chunk(&token, &id, 100, &rest).await;
    assert_eq!(sent.json()["received"], 200);
    let done = env
        .post_json(
            &format!("/mods/uploads/{id}/complete"),
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(done.status, 201, "{:?}", done.json());
    let stored = env
        .dir
        .path()
        .join("pending/brazil-livery/1.0/brazil-livery.zip");
    assert_eq!(std::fs::read(stored).unwrap(), world.content);
}

#[then(regex = r#"^the public catalog shows "([^"]*)" version "([^"]*)"$"#)]
async fn catalog_shows(world: &mut ModsWorld, mod_id: String, version: String) {
    let catalog = world.catalog().await;
    let entries = catalog["entries"].as_array().unwrap();
    let matching: Vec<_> = entries
        .iter()
        .filter(|e| e["id"] == mod_id.as_str())
        .collect();
    assert_eq!(matching.len(), 1, "one entry per mod: {catalog}");
    assert_eq!(matching[0]["version"], version.as_str());
}

#[then(regex = r#"^"My mods" lists both versions of "([^"]*)"$"#)]
async fn my_mods_both(world: &mut ModsWorld, mod_id: String) {
    let mine = world.mine().await;
    let versions: Vec<&str> = mine["mods"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["id"] == mod_id.as_str())
        .flat_map(|m| m["versions"].as_array().unwrap().iter())
        .map(|v| v["version"].as_str().unwrap())
        .collect();
    assert_eq!(versions, ["1.0", "1.1"]);
}

#[then(regex = r"^the server answers 206 with those 100 bytes$")]
async fn answers_206(world: &mut ModsWorld) {
    assert_eq!(world.last().status, 206);
    assert_eq!(world.last().bytes, world.content[..100]);
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    ModsWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/mod-hosting.feature"
        ))
        .await;
}
