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
        let state = AppState {
            clock: clock.clone(),
            health: store.clone(),
            auth: auth_service(
                store.clone(),
                clock.clone(),
                Arc::new(FakeDiscord::new("rider")),
            ),
            stars: star_service(store.clone()),
            limits: RateLimits::new(clock.clone()),
            comments: comments_state(store.clone(), clock.clone()),
            downloads: downloads_state(store.clone(), clock.clone()),
            trust_proxy_headers: false,
            mods,
        };
        Self {
            app: build_router(state),
            clock,
            store,
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
