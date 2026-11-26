//! Shared harness of the mod hosting tests: the real router over in-memory
//! SQLite, a temp storage directory and a fake clock. No network, no real time.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, HeaderMap, Method, Request};
use axum::Router;
use chrono::{DateTime, Duration, Utc};
use community_api::testing::{
    auth_service, comments_state, downloads_state, mod_hosting, star_service, FakeClock,
    FakeDiscord,
};
use community_api::{build_router, AppState, RateLimits};
use community_application::moderation::ModerationRepository;
use community_application::mods::ModLimits;
use community_application::ports::{Hasher, SessionRepository, UserRepository, UserUpsert};
use community_domain::{DiscordId, Session, UserId};
use community_infrastructure::{Sha256Hasher, SqliteStore};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

pub const ADMIN_DISCORD_ID: &str = "999";

/// A tiny but valid PNG header (only the magic bytes matter to the server).
pub const PNG: [u8; 12] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

pub struct Reply {
    pub status: u16,
    pub headers: HeaderMap,
    pub bytes: Vec<u8>,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }

    pub fn header(&self, name: &str) -> String {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    }

    /// Every `Set-Cookie` header value.
    pub fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(str::to_owned)
            .collect()
    }

    pub fn error_code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

pub struct Env {
    pub app: Router,
    pub clock: Arc<FakeClock>,
    pub store: Arc<SqliteStore>,
    pub discord: Arc<FakeDiscord>,
    /// The moderation service (admins: [`ADMIN_DISCORD_ID`]); tests call admin actions on it directly.
    pub moderation: Arc<community_application::ModerationService>,
    pub dir: tempfile::TempDir,
}

pub fn limits(max: u64, quota: u64, per_day: u32) -> ModLimits {
    ModLimits {
        max_upload_bytes: max,
        user_quota_bytes: quota,
        uploads_per_day: per_day,
    }
}

impl Env {
    pub async fn new(limits: ModLimits) -> Self {
        let clock = Arc::new(FakeClock::at_epoch_plus(1_700_000_000));
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        let store = Arc::new(store);
        let dir = tempfile::tempdir().unwrap();
        let mods = mod_hosting(
            store.clone(),
            clock.clone(),
            dir.path(),
            vec![DiscordId::parse(ADMIN_DISCORD_ID).unwrap()],
            limits,
        );
        // The directories exist before the router is built, like in `main`.
        community_infrastructure::LocalModStorage::new(dir.path())
            .ensure_dirs()
            .await
            .unwrap();
        let moderation = community_api::testing::moderation_service(
            store.clone(),
            clock.clone(),
            vec![DiscordId::parse(ADMIN_DISCORD_ID).unwrap()],
        );
        let discord = Arc::new(FakeDiscord::new("rider"));
        let state = AppState {
            clock: clock.clone(),
            health: store.clone(),
            auth: auth_service(store.clone(), clock.clone(), discord.clone()),
            stars: star_service(store.clone()),
            limits: RateLimits::new(clock.clone()),
            moderation: moderation.clone(),
            comments: comments_state(store.clone(), clock.clone()),
            downloads: downloads_state(store.clone(), clock.clone()),
            trust_proxy_headers: false,
            mods,
        };
        Self {
            app: build_router(state),
            clock,
            store,
            discord,
            moderation,
            dir,
        }
    }

    pub fn advance(&self, by: Duration) {
        self.clock.advance(by);
    }

    pub fn now(&self) -> DateTime<Utc> {
        use community_application::ports::Clock;
        self.clock.now()
    }

    /// Creates a user with an active session and returns its bearer token.
    pub async fn user(&self, name: &str, discord_id: &str) -> String {
        let now = self.now();
        let user = self
            .store
            .upsert(&UserUpsert {
                new_id: UserId(format!("user-{name}")),
                discord_id: DiscordId::parse(discord_id).unwrap(),
                username: name.into(),
                global_name: None,
                avatar_hash: None,
                now,
            })
            .await
            .unwrap();
        let token = format!("token-{name}");
        self.store
            .create(&Session {
                id: format!("session-{name}"),
                user_id: user.id,
                token_hash: Sha256Hasher.hash(&token),
                created_at: now,
                expires_at: Session::expiry_from(now),
                revoked_at: None,
            })
            .await
            .unwrap();
        token
    }

    pub async fn admin(&self) -> String {
        self.user("admin", ADMIN_DISCORD_ID).await
    }

    /// Posts a comment; returns the raw reply.
    pub async fn comment(&self, token: &str, mod_id: &str, body: &str) -> Reply {
        self.post_json(
            &format!("/mods/{mod_id}/comments"),
            Some(token),
            json!({ "body": body }),
        )
        .await
    }

    /// Posts a comment and returns its id (panics unless it answers 201).
    pub async fn comment_id(&self, token: &str, mod_id: &str, body: &str) -> String {
        let reply = self.comment(token, mod_id, body).await;
        assert_eq!(reply.status, 201, "{:?}", reply.json());
        reply.json()["id"].as_str().unwrap().to_owned()
    }

    /// Reports a comment; returns the raw reply.
    pub async fn report(&self, token: &str, mod_id: &str, comment_id: &str, reason: &str) -> Reply {
        self.post_json(
            &format!("/mods/{mod_id}/comments/{comment_id}/reports"),
            Some(token),
            json!({ "reason": reason }),
        )
        .await
    }

    /// Bodies of the comments `GET` returns for the mod (anonymous, up to 50).
    pub async fn visible_comments(&self, mod_id: &str) -> Vec<String> {
        let reply = self
            .get(&format!("/mods/{mod_id}/comments?limit=50"), None)
            .await;
        assert_eq!(reply.status, 200);
        reply.json()["comments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["body"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The `User` of a name created with [`Env::user`].
    pub async fn user_row(&self, name: &str) -> community_domain::User {
        self.store
            .get(&UserId(format!("user-{name}")))
            .await
            .unwrap()
            .unwrap()
    }

    pub async fn send(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        headers: &[(&str, &str)],
        body: Vec<u8>,
        json_body: bool,
    ) -> Reply {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if json_body {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let mut request = builder.body(Body::from(body)).unwrap();
        let addr: SocketAddr = "10.0.0.1:4000".parse().unwrap();
        request.extensions_mut().insert(ConnectInfo(addr));
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            bytes: bytes.to_vec(),
        }
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> Reply {
        self.send(Method::GET, uri, token, &[], Vec::new(), false)
            .await
    }

    pub async fn post_json(&self, uri: &str, token: Option<&str>, body: Value) -> Reply {
        self.send(
            Method::POST,
            uri,
            token,
            &[],
            body.to_string().into_bytes(),
            true,
        )
        .await
    }

    pub async fn put_chunk(&self, token: &str, id: &str, offset: u64, bytes: &[u8]) -> Reply {
        self.send(
            Method::PUT,
            &format!("/mods/uploads/{id}/chunks?offset={offset}"),
            Some(token),
            &[],
            bytes.to_vec(),
            false,
        )
        .await
    }

    pub fn create_body(mod_id: &str, version: &str, size: u64) -> Value {
        json!({
            "modId": mod_id,
            "title": "Brazil Livery",
            "description": "A livery for the Brazil round",
            "category": "liveries",
            "game": "MotoGP 26",
            "version": version,
            "fileName": "brazil-livery.zip",
            "size": size,
            "ownershipConfirmed": true,
        })
    }

    /// Starts an upload; returns the raw reply.
    pub async fn start(&self, token: &str, mod_id: &str, version: &str, size: u64) -> Reply {
        self.post_json(
            "/mods/uploads",
            Some(token),
            Self::create_body(mod_id, version, size),
        )
        .await
    }

    /// Uploads `content` completely (in `chunk` byte pieces) and completes it; returns the version JSON.
    pub async fn publish(
        &self,
        token: &str,
        mod_id: &str,
        version: &str,
        content: &[u8],
        chunk: usize,
    ) -> Value {
        let started = self
            .start(token, mod_id, version, content.len() as u64)
            .await;
        assert_eq!(started.status, 201, "{:?}", started.json());
        let id = started.json()["uploadId"].as_str().unwrap().to_owned();
        let mut offset = 0;
        for piece in content.chunks(chunk) {
            let reply = self.put_chunk(token, &id, offset, piece).await;
            assert_eq!(reply.status, 200, "{:?}", reply.json());
            offset += piece.len() as u64;
        }
        let done = self
            .post_json(
                &format!("/mods/uploads/{id}/complete"),
                Some(token),
                json!({}),
            )
            .await;
        assert_eq!(done.status, 201, "{:?}", done.json());
        done.json()
    }

    pub async fn approve(&self, admin: &str, mod_id: &str, version: &str) -> Reply {
        self.post_json(
            &format!("/admin/mods/{mod_id}/versions/{version}/approve"),
            Some(admin),
            json!({}),
        )
        .await
    }

    pub async fn reject(&self, admin: &str, mod_id: &str, version: &str, reason: &str) -> Reply {
        self.post_json(
            &format!("/admin/mods/{mod_id}/versions/{version}/reject"),
            Some(admin),
            json!({ "reason": reason }),
        )
        .await
    }
}

/// An admin web browser session: the `Cookie` header value and the CSRF token.
#[derive(Clone)]
pub struct AdminBrowser {
    pub cookie: String,
    pub csrf: String,
}

/// Percent-encodes one form value.
pub fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The value of `name=` inside a `Set-Cookie` header list.
pub fn cookie_from(set_cookies: &[String], name: &str) -> Option<String> {
    set_cookies.iter().find_map(|line| {
        let first = line.split(';').next()?;
        let (key, value) = first.split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

impl Env {
    /// `GET /admin/login`: the reply and the OAuth `state` it sent to Discord.
    pub async fn admin_login_start(&self) -> (Reply, String) {
        let reply = self.get("/admin/login", None).await;
        let state = cookie_from(&reply.set_cookies(), "admin_login").unwrap_or_default();
        (reply, state)
    }

    /// Discord's redirect back to the callback, with an optional `Cookie` header.
    pub async fn admin_callback(&self, state: &str, cookie: Option<&str>) -> Reply {
        let uri = format!("/auth/discord/callback?code=discord-code&state={state}");
        let headers: Vec<(&str, &str)> = cookie.map(|c| ("cookie", c)).into_iter().collect();
        self.send(Method::GET, &uri, None, &headers, Vec::new(), false)
            .await
    }

    /// The full admin sign-in as the given Discord user; the callback reply.
    pub async fn admin_sign_in(&self, discord_id: &str, username: &str) -> Reply {
        self.discord
            .set_user(community_application::ports::DiscordUser {
                id: discord_id.into(),
                username: username.into(),
                global_name: None,
                avatar: None,
            });
        let (_, state) = self.admin_login_start().await;
        self.admin_callback(&state, Some(&format!("admin_login={state}")))
            .await
    }

    /// Signs the configured admin in through the real flow.
    pub async fn admin_browser(&self) -> AdminBrowser {
        let reply = self.admin_sign_in(ADMIN_DISCORD_ID, "Mod").await;
        assert_eq!(reply.status, 200, "{}", reply.text());
        let token = cookie_from(&reply.set_cookies(), "admin_session").expect("session cookie");
        let cookie = format!("admin_session={token}");
        let page = self
            .send(
                Method::GET,
                "/admin",
                None,
                &[("cookie", &cookie)],
                Vec::new(),
                false,
            )
            .await;
        let html = page.text();
        let marker = "name=\"csrf\" value=\"";
        let start = html.find(marker).expect("a csrf field") + marker.len();
        let csrf = html[start..].split('"').next().unwrap().to_owned();
        AdminBrowser { cookie, csrf }
    }

    pub async fn admin_get(&self, uri: &str, browser: &AdminBrowser) -> Reply {
        self.send(
            Method::GET,
            uri,
            None,
            &[("cookie", &browser.cookie)],
            Vec::new(),
            false,
        )
        .await
    }

    /// POSTs a form with the browser's CSRF token plus `fields`.
    pub async fn admin_post(
        &self,
        uri: &str,
        browser: &AdminBrowser,
        fields: &[(&str, &str)],
    ) -> Reply {
        let mut all = vec![("csrf", browser.csrf.as_str())];
        all.extend_from_slice(fields);
        self.admin_post_form(uri, Some(&browser.cookie), &all).await
    }

    /// POSTs exactly the given form fields (no automatic CSRF token).
    pub async fn admin_post_form(
        &self,
        uri: &str,
        cookie: Option<&str>,
        fields: &[(&str, &str)],
    ) -> Reply {
        let body = fields
            .iter()
            .map(|(k, v)| format!("{k}={}", form_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let mut headers = vec![("content-type", "application/x-www-form-urlencoded")];
        if let Some(cookie) = cookie {
            headers.push(("cookie", cookie));
        }
        self.send(Method::POST, uri, None, &headers, body.into_bytes(), false)
            .await
    }

    /// Publishes a mod version with one screenshot, as a creator.
    pub async fn publish_with_screenshot(
        &self,
        token: &str,
        mod_id: &str,
        version: &str,
        content: &[u8],
    ) {
        let started = self
            .start(token, mod_id, version, content.len() as u64)
            .await;
        assert_eq!(started.status, 201, "{}", started.text());
        let id = started.json()["uploadId"].as_str().unwrap().to_owned();
        assert_eq!(self.put_chunk(token, &id, 0, content).await.status, 200);
        let shot = self
            .send(
                Method::POST,
                &format!("/mods/uploads/{id}/screenshots"),
                Some(token),
                &[],
                PNG.to_vec(),
                false,
            )
            .await;
        assert_eq!(shot.status, 201, "{}", shot.text());
        let done = self
            .post_json(
                &format!("/mods/uploads/{id}/complete"),
                Some(token),
                json!({}),
            )
            .await;
        assert_eq!(done.status, 201, "{}", done.text());
    }
}

impl Env {
    /// The moderator's user row, created (with a session) when it does not exist yet.
    pub async fn moderation_admin(&self) -> community_domain::User {
        let id = DiscordId::parse(ADMIN_DISCORD_ID).unwrap();
        if let Some(user) = self.store.find_user_by_discord_id(&id).await.unwrap() {
            return user;
        }
        self.user("admin", ADMIN_DISCORD_ID).await;
        self.user_row("admin").await
    }
}
