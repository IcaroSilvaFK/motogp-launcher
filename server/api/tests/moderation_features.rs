//! BDD runner (cucumber, `harness = false`) for `features/moderation.feature`.
//!
//! Every user step drives the real router (in-memory SQLite, temp storage,
//! fake clock): users use the JSON API, the admin uses the HTML admin pages
//! (cookie session and CSRF token). The run exits non-zero when a step fails,
//! is missing or ambiguous.

mod mod_common;

use std::collections::HashMap;
use std::fmt;

use axum::http::Method;
use chrono::Duration;
use community_application::moderation::ModerationRepository;
use community_domain::User;
use cucumber::{given, then, when, World};
use mod_common::{cookie_from, limits, AdminBrowser, Env, ADMIN_DISCORD_ID, PNG};
use serde_json::json;

#[derive(Default, World)]
struct ModerationWorld {
    env: Option<Env>,
    /// Display name -> bearer token.
    tokens: HashMap<String, String>,
    /// Status of the last request.
    last_status: Option<u16>,
    /// The comment the last "posted" step created: `(mod id, comment id)`.
    comment: Option<(String, String)>,
    /// Upload id started by each user.
    uploads: HashMap<String, String>,
    /// The admin's browser session (created on first use).
    browser: Option<AdminBrowser>,
    /// Body and cookies of the last admin page or sign-in reply.
    last_page: String,
    last_cookies: Vec<String>,
}

impl fmt::Debug for ModerationWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModerationWorld")
            .field("last_status", &self.last_status)
            .finish_non_exhaustive()
    }
}

impl ModerationWorld {
    fn env(&self) -> &Env {
        self.env
            .as_ref()
            .expect("the Background creates the server")
    }

    fn token(&self, name: &str) -> String {
        self.tokens[name].clone()
    }

    fn comment(&self) -> (String, String) {
        self.comment.clone().expect("a comment was posted")
    }

    async fn admin(&self) -> User {
        self.env().user_row("Mod").await
    }

    async fn user(&self, name: &str) -> User {
        self.env().user_row(name).await
    }

    async fn browser(&mut self) -> AdminBrowser {
        if self.browser.is_none() {
            let browser = self.env().admin_browser().await;
            self.browser = Some(browser);
        }
        self.browser.clone().unwrap()
    }

    /// POSTs an admin form (with the CSRF token) and remembers the status.
    async fn admin_form(&mut self, uri: &str, fields: &[(&str, &str)]) -> u16 {
        let browser = self.browser().await;
        let reply = self.env().admin_post(uri, &browser, fields).await;
        self.last_status = Some(reply.status);
        reply.status
    }

    async fn open_admin_page(&mut self) {
        let browser = self.browser().await;
        let reply = self.env().admin_get("/admin", &browser).await;
        assert_eq!(reply.status, 200);
        self.last_page = reply.text();
    }

    async fn report(&mut self, name: &str, reason: &str) -> u16 {
        let (mod_id, id) = self.comment();
        let token = self.token(name);
        let status = self.env().report(&token, &mod_id, &id, reason).await.status;
        self.last_status = Some(status);
        status
    }

    async fn visible(&self, mod_id: &str) -> Vec<String> {
        self.env().visible_comments(mod_id).await
    }
}

// ---- Given ----

#[given(
    regex = r#"^a community server with the users "([^"]*)" and "([^"]*)" and the admin "([^"]*)"$"#
)]
async fn community_server(
    world: &mut ModerationWorld,
    first: String,
    second: String,
    admin: String,
) {
    let env = Env::new(limits(1_000_000, 10_000_000, 20)).await;
    for (index, name) in [first, second].into_iter().enumerate() {
        let token = env.user(&name, &(101 + index).to_string()).await;
        world.tokens.insert(name, token);
    }
    let token = env.user(&admin, ADMIN_DISCORD_ID).await;
    world.tokens.insert(admin, token);
    world.env = Some(env);
}

#[given(regex = r#"^"([^"]*)" posted "([^"]*)" on "([^"]*)"$"#)]
async fn posted(world: &mut ModerationWorld, name: String, text: String, mod_id: String) {
    let token = world.token(&name);
    let id = world.env().comment_id(&token, &mod_id, &text).await;
    world.comment = Some((mod_id, id));
    // Later comments always sort after this one.
    world.env().advance(Duration::seconds(1));
}

#[given(regex = r#"^"([^"]*)" reported that comment as "([^"]*)"$"#)]
async fn reported(world: &mut ModerationWorld, name: String, reason: String) {
    assert_eq!(world.report(&name, &reason).await, 204);
}

#[given(regex = r#"^the admin hid that comment with the reason "([^"]*)"$"#)]
async fn admin_hid(world: &mut ModerationWorld, reason: String) {
    hide(world, reason).await;
}

#[given(regex = r#"^the admin banned "([^"]*)" with the reason "([^"]*)"$"#)]
async fn admin_banned(world: &mut ModerationWorld, name: String, reason: String) {
    ban(world, name, reason).await;
}

#[given(regex = r#"^"([^"]*)" started an upload of "([^"]*)" version "([^"]*)"$"#)]
async fn started_upload(
    world: &mut ModerationWorld,
    name: String,
    mod_id: String,
    version: String,
) {
    let token = world.token(&name);
    let reply = world.env().start(&token, &mod_id, &version, 11).await;
    assert_eq!(reply.status, 201, "{:?}", reply.json());
    let id = reply.json()["uploadId"].as_str().unwrap().to_owned();
    world.uploads.insert(name, id);
}

#[given(regex = r#"^"([^"]*)" published "([^"]*)" version "([^"]*)" with a screenshot$"#)]
async fn published_with_screenshot(
    world: &mut ModerationWorld,
    name: String,
    mod_id: String,
    version: String,
) {
    let token = world.token(&name);
    world
        .env()
        .publish_with_screenshot(&token, &mod_id, &version, b"zipdata")
        .await;
}

#[given(regex = r"^the admin is signed in to the admin page$")]
async fn admin_signed_in(world: &mut ModerationWorld) {
    world.browser().await;
}

// ---- When (admin web) ----

#[when(regex = r"^the admin signs in at /admin/login$")]
async fn admin_signs_in(world: &mut ModerationWorld) {
    sign_in_at_admin_login(world, ADMIN_DISCORD_ID, "Mod").await;
}

#[when(regex = r"^a non-admin signs in at /admin/login$")]
async fn non_admin_signs_in(world: &mut ModerationWorld) {
    sign_in_at_admin_login(world, "555", "Rider").await;
}

async fn sign_in_at_admin_login(world: &mut ModerationWorld, discord_id: &str, name: &str) {
    let reply = world.env().admin_sign_in(discord_id, name).await;
    world.last_status = Some(reply.status);
    world.last_cookies = reply.set_cookies();
    world.last_page = reply.text();
}

#[when(regex = r"^the admin opens the admin page$")]
async fn admin_opens_page(world: &mut ModerationWorld) {
    world.open_admin_page().await;
}

#[when(regex = r"^the admin posts a hide form for that comment without a csrf token$")]
async fn hide_without_csrf(world: &mut ModerationWorld) {
    let (_, id) = world.comment();
    let browser = world.browser().await;
    let reply = world
        .env()
        .admin_post_form(
            &format!("/admin/comments/{id}/hide"),
            Some(&browser.cookie),
            &[("reason", "no token")],
        )
        .await;
    world.last_status = Some(reply.status);
}

#[when(regex = r#"^the admin approves "([^"]*)" version "([^"]*)" on the admin page$"#)]
async fn admin_approves(world: &mut ModerationWorld, mod_id: String, version: String) {
    world
        .admin_form(&format!("/admin/review/{mod_id}/{version}/approve"), &[])
        .await;
}

#[when(
    regex = r#"^the admin rejects "([^"]*)" version "([^"]*)" on the admin page with the reason "([^"]*)"$"#
)]
async fn admin_rejects(
    world: &mut ModerationWorld,
    mod_id: String,
    version: String,
    reason: String,
) {
    world
        .admin_form(
            &format!("/admin/review/{mod_id}/{version}/reject"),
            &[("reason", &reason)],
        )
        .await;
}

// ---- Then (admin web) ----

#[then(regex = r"^an HttpOnly, Secure, SameSite=Strict admin session cookie is set$")]
async fn session_cookie_set(world: &mut ModerationWorld) {
    let cookie = world
        .last_cookies
        .iter()
        .find(|c| c.starts_with("admin_session="))
        .expect("an admin_session cookie");
    for part in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/admin"] {
        assert!(cookie.contains(part), "{cookie}");
    }
}

#[then(regex = r"^no admin session cookie is set$")]
async fn no_session_cookie(world: &mut ModerationWorld) {
    assert!(cookie_from(&world.last_cookies, "admin_session").is_none());
}

#[then(regex = r"^the admin page opens with that session$")]
async fn admin_page_opens(world: &mut ModerationWorld) {
    let token = cookie_from(&world.last_cookies, "admin_session").expect("session cookie");
    let reply = world
        .env()
        .send(
            Method::GET,
            "/admin",
            None,
            &[("cookie", &format!("admin_session={token}"))],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(reply.status, 200);
    assert!(reply.text().contains("Open reports"));
}

#[then(regex = r#"^the admin page shows "([^"]*)"$"#)]
async fn page_shows(world: &mut ModerationWorld, text: String) {
    world.open_admin_page().await;
    assert!(world.last_page.contains(&text), "{text} not on the page");
}

#[then(
    regex = r#"^the pending version "([^"]*)" shows its metadata, screenshots and a download link$"#
)]
async fn pending_version_shown(world: &mut ModerationWorld, mod_id: String) {
    let page = &world.last_page;
    assert!(page.contains("Brazil Livery"), "title");
    assert!(page.contains("MotoGP 26"), "game");
    assert!(page.contains(&format!("/admin/mods/{mod_id}/versions/1.0/screenshots/0")));
    assert!(page.contains(&format!("/admin/mods/{mod_id}/versions/1.0/file")));
}

#[then(regex = r#"^the catalog lists "([^"]*)" version "([^"]*)"$"#)]
async fn catalog_lists(world: &mut ModerationWorld, mod_id: String, version: String) {
    let catalog = world.env().get("/catalog", None).await.json();
    let entry = catalog["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == mod_id)
        .expect("the mod is in the catalog");
    assert_eq!(entry["version"], version.as_str());
}

// ---- When ----

#[when(regex = r#"^"([^"]*)" reports that comment as "([^"]*)"$"#)]
async fn reports(world: &mut ModerationWorld, name: String, reason: String) {
    world.report(&name, &reason).await;
}

#[when(regex = r#"^the admin hides that comment with the reason "([^"]*)"$"#)]
async fn admin_hides(world: &mut ModerationWorld, reason: String) {
    hide(world, reason).await;
}

async fn hide(world: &mut ModerationWorld, reason: String) {
    let (_, id) = world.comment();
    let status = world
        .admin_form(
            &format!("/admin/comments/{id}/hide"),
            &[("reason", &reason)],
        )
        .await;
    assert_eq!(status, 303);
}

#[when(regex = r"^the admin restores that comment$")]
async fn admin_restores(world: &mut ModerationWorld) {
    let (_, id) = world.comment();
    let status = world
        .admin_form(&format!("/admin/comments/{id}/restore"), &[])
        .await;
    assert_eq!(status, 303);
}

#[when(regex = r#"^the admin bans "([^"]*)" with the reason "([^"]*)"$"#)]
async fn admin_bans(world: &mut ModerationWorld, name: String, reason: String) {
    ban(world, name, reason).await;
}

async fn ban(world: &mut ModerationWorld, name: String, reason: String) {
    let target = world.user(&name).await;
    let uri = format!("/admin/users/{}/ban", target.id.as_str());
    let status = world.admin_form(&uri, &[("reason", &reason)]).await;
    assert_eq!(status, 303);
}

#[when(regex = r#"^the admin unbans "([^"]*)"$"#)]
async fn admin_unbans(world: &mut ModerationWorld, name: String) {
    let target = world.user(&name).await;
    let uri = format!("/admin/users/{}/unban", target.id.as_str());
    let status = world.admin_form(&uri, &[]).await;
    assert_eq!(status, 303);
}

// ---- Then ----

#[then(regex = r"^the server answers (\d+)$")]
async fn server_answers(world: &mut ModerationWorld, status: u16) {
    assert_eq!(world.last_status, Some(status));
}

#[then(regex = r"^the admin queue has (\d+) reported comments? with (\d+) reports?$")]
async fn queue_has(world: &mut ModerationWorld, comments: usize, reports: usize) {
    let admin = world.admin().await;
    let groups = world.env().moderation.open_reports(&admin).await.unwrap();
    assert_eq!(groups.len(), comments);
    assert_eq!(groups[0].report_count(), reports);
}

#[then(regex = r"^the admin queue is empty$")]
async fn queue_empty(world: &mut ModerationWorld) {
    let admin = world.admin().await;
    assert!(world
        .env()
        .moderation
        .open_reports(&admin)
        .await
        .unwrap()
        .is_empty());
}

#[then(regex = r#"^"([^"]*)" shows no comments$"#)]
async fn shows_no_comments(world: &mut ModerationWorld, mod_id: String) {
    assert!(world.visible(&mod_id).await.is_empty());
}

#[then(regex = r#"^"([^"]*)" shows the comment "([^"]*)"$"#)]
async fn shows_comment(world: &mut ModerationWorld, mod_id: String, text: String) {
    assert_eq!(world.visible(&mod_id).await, [text]);
}

#[then(regex = r#"^the audit log records "([^"]*)" by "([^"]*)" with the reason "([^"]*)"$"#)]
async fn audit_with_reason(
    world: &mut ModerationWorld,
    action: String,
    admin: String,
    reason: String,
) {
    let entry = latest_audit(world, &action, &admin).await;
    assert_eq!(entry.reason.as_deref(), Some(reason.as_str()));
}

#[then(regex = r#"^the audit log records "([^"]*)" by "([^"]*)" with no reason$"#)]
async fn audit_without_reason(world: &mut ModerationWorld, action: String, admin: String) {
    let entry = latest_audit(world, &action, &admin).await;
    assert_eq!(entry.reason, None);
}

async fn latest_audit(
    world: &ModerationWorld,
    action: &str,
    admin: &str,
) -> community_domain::moderation::AuditEntry {
    let entries = world.env().store.recent_audit(20).await.unwrap();
    let entry = entries
        .into_iter()
        .find(|entry| entry.action.as_str() == action)
        .unwrap_or_else(|| panic!("no {action} audit entry"));
    assert_eq!(entry.admin_name.as_deref(), Some(admin));
    // The time comes from the (fake) clock, so it is always recorded.
    assert!(entry.created_at.timestamp() >= 1_700_000_000);
    entry
}

#[then(regex = r#"^"([^"]*)" reports that comment as "([^"]*)" and the server answers (\d+)$"#)]
async fn reports_and_answers(
    world: &mut ModerationWorld,
    name: String,
    reason: String,
    status: u16,
) {
    assert_eq!(world.report(&name, &reason).await, status);
}

#[then(regex = r#"^"([^"]*)" can comment again$"#)]
async fn can_comment_again(world: &mut ModerationWorld, name: String) {
    let token = world.token(&name);
    let reply = world
        .env()
        .comment(&token, "livery-pack", "I am back")
        .await;
    assert_eq!(reply.status, 201, "{:?}", reply.json());
}

#[then(regex = r#"^"([^"]*)" is refused with 403 banned when they (.+)$"#)]
async fn refused_when(world: &mut ModerationWorld, name: String, action: String) {
    let token = world.token(&name);
    let (mod_id, comment_id) = world.comment.clone().unwrap_or_default();
    let upload = world.uploads.get(&name).cloned().unwrap_or_default();
    let env = world.env();
    let empty = Vec::new;
    let reply = match action.as_str() {
        "comment" => env.comment(&token, &mod_id, "hello").await,
        "edit their comment" => {
            let body = json!({ "body": "edited" }).to_string().into_bytes();
            env.send(
                Method::PATCH,
                &format!("/mods/{mod_id}/comments/{comment_id}"),
                Some(&token),
                &[],
                body,
                true,
            )
            .await
        }
        "delete their comment" => {
            env.send(
                Method::DELETE,
                &format!("/mods/{mod_id}/comments/{comment_id}"),
                Some(&token),
                &[],
                empty(),
                false,
            )
            .await
        }
        "star a mod" => {
            env.send(
                Method::PUT,
                "/mods/livery-pack/star",
                Some(&token),
                &[],
                empty(),
                false,
            )
            .await
        }
        "unstar a mod" => {
            env.send(
                Method::DELETE,
                "/mods/livery-pack/star",
                Some(&token),
                &[],
                empty(),
                false,
            )
            .await
        }
        "report a comment" => env.report(&token, &mod_id, &comment_id, "spam").await,
        "start an upload" => env.start(&token, "brazil-livery", "2.0", 11).await,
        "send a chunk" => env.put_chunk(&token, &upload, 0, b"hello world").await,
        "add a screenshot" => {
            env.send(
                Method::POST,
                &format!("/mods/uploads/{upload}/screenshots"),
                Some(&token),
                &[],
                PNG.to_vec(),
                false,
            )
            .await
        }
        "complete the upload" => {
            env.post_json(
                &format!("/mods/uploads/{upload}/complete"),
                Some(&token),
                json!({}),
            )
            .await
        }
        "abort the upload" => {
            env.send(
                Method::DELETE,
                &format!("/mods/uploads/{upload}"),
                Some(&token),
                &[],
                empty(),
                false,
            )
            .await
        }
        other => panic!("unknown action: {other}"),
    };
    assert_eq!(reply.status, 403, "{action}: {:?}", reply.json());
    assert_eq!(reply.error_code(), "banned", "{action}");
}

#[tokio::main]
async fn main() {
    // Fail on failed, skipped (missing) and ambiguous steps alike.
    ModerationWorld::cucumber()
        .fail_on_skipped()
        .run_and_exit(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/features/moderation.feature"
        ))
        .await;
}
