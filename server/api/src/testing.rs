//! Helpers shared by unit and integration tests (no network, no real time).

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use community_application::comments::CommentsService;
use community_application::ports::{
    AccessToken, Clock, DiscordOAuth, DiscordUser, Hasher, PortResult, SessionRepository,
    UserRepository, UserUpsert,
};
use community_application::{AppError, AuthService, StarService};
use community_domain::{DiscordId, Session, User, UserId};

use crate::comments::CommentsState;
use community_infrastructure::{OsTokenGenerator, Sha256Hasher, SqliteStore};

/// A clock that only moves when told to.
pub struct FakeClock(Mutex<DateTime<Utc>>);

impl FakeClock {
    pub fn new(start: DateTime<Utc>) -> Self {
        Self(Mutex::new(start))
    }

    pub fn at_epoch_plus(secs: i64) -> Self {
        Self::new(DateTime::from_timestamp(secs, 0).unwrap())
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.0.lock().unwrap();
        *now += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

/// Discord stand-in for tests: no network, a configurable profile.
pub struct FakeDiscord {
    user: Mutex<DiscordUser>,
    fail: Mutex<bool>,
}

impl FakeDiscord {
    pub fn new(username: &str) -> Self {
        Self {
            user: Mutex::new(DiscordUser {
                id: "123456789012345678".into(),
                username: username.into(),
                global_name: None,
                avatar: None,
            }),
            fail: Mutex::new(false),
        }
    }

    pub fn set_user(&self, user: DiscordUser) {
        *self.user.lock().unwrap() = user;
    }

    pub fn set_failing(&self, fail: bool) {
        *self.fail.lock().unwrap() = fail;
    }
}

#[async_trait::async_trait]
impl DiscordOAuth for FakeDiscord {
    fn authorize_url(&self, state: &str, code_challenge: &str) -> String {
        format!(
            "https://discord.test/oauth2/authorize?state={state}&code_challenge={code_challenge}"
        )
    }

    async fn exchange_code(&self, _code: &str, _verifier: &str) -> PortResult<AccessToken> {
        if *self.fail.lock().unwrap() {
            return Err(AppError::DiscordUnavailable);
        }
        Ok(AccessToken("fake-access-token".into()))
    }

    async fn fetch_user(&self, _token: &AccessToken) -> PortResult<DiscordUser> {
        Ok(self.user.lock().unwrap().clone())
    }
}

/// Builds the comments state (real service and write limiter) over the store.
pub fn comments_state(store: Arc<SqliteStore>, clock: Arc<dyn Clock>) -> CommentsState {
    CommentsState::new(
        Arc::new(CommentsService {
            clock: clock.clone(),
            tokens: Arc::new(OsTokenGenerator),
            comments: store,
        }),
        clock,
    )
}

/// Creates (or refreshes) a user and an active session; returns the user and
/// the bearer token. `display_name` becomes the Discord global name.
pub async fn sign_in_user(
    store: &SqliteStore,
    clock: &dyn Clock,
    discord_id: &str,
    display_name: &str,
) -> (User, String) {
    let now = clock.now();
    let user = store
        .upsert(&UserUpsert {
            new_id: UserId(format!("user-{discord_id}")),
            discord_id: DiscordId::parse(discord_id).expect("a numeric discord id"),
            username: format!("{display_name}-username"),
            global_name: Some(display_name.to_owned()),
            avatar_hash: None,
            now,
        })
        .await
        .unwrap();
    let token = format!("test-token-{discord_id}");
    store
        .create(&Session {
            id: format!("session-{discord_id}"),
            user_id: user.id.clone(),
            token_hash: Sha256Hasher.hash(&token),
            created_at: now,
            expires_at: Session::expiry_from(now),
            revoked_at: None,
        })
        .await
        .unwrap();
    (user, token)
}

/// Builds the real [`AuthService`] over the SQLite store with the given fakes.
pub fn auth_service(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
    discord: Arc<dyn DiscordOAuth>,
) -> Arc<AuthService> {
    Arc::new(AuthService {
        clock,
        tokens: Arc::new(OsTokenGenerator),
        hasher: Arc::new(Sha256Hasher),
        users: store.clone(),
        sessions: store.clone(),
        flows: store.clone(),
        codes: store,
        discord,
    })
}

/// Builds the real [`StarService`] over the SQLite store.
pub fn star_service(store: Arc<SqliteStore>) -> Arc<StarService> {
    Arc::new(StarService::new(store))
}

/// Download counts over the SQLite store (real hasher, default 20/minute limiter).
pub fn downloads_state(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
) -> crate::downloads::DownloadsState {
    let service = Arc::new(community_application::DownloadService {
        clock: clock.clone(),
        hasher: Arc::new(Sha256Hasher),
        downloads: store,
    });
    crate::downloads::DownloadsState::new(service, clock)
}

/// Wires the real mod hosting use cases over the SQLite store and a storage
/// directory (tests pass a temp dir). Admins are the given Discord ids.
pub fn mod_hosting(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
    storage_root: &std::path::Path,
    admin_discord_ids: Vec<community_domain::DiscordId>,
    limits: community_application::mods::ModLimits,
) -> crate::mods::ModHosting {
    let storage = community_infrastructure::LocalModStorage::new(storage_root);
    let files_dir = storage.public_dir();
    crate::mods::ModHosting {
        service: Arc::new(community_application::ModHostingService {
            clock,
            tokens: Arc::new(OsTokenGenerator),
            repo: store.clone(),
            storage: Arc::new(storage),
            audit: store,
            admin_discord_ids,
            limits,
        }),
        public_base_url: "https://community.test".into(),
        files_dir,
    }
}

/// Mod hosting for tests that never touch it (the directory does not exist).
pub fn unused_mod_hosting(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
) -> crate::mods::ModHosting {
    mod_hosting(
        store,
        clock,
        std::path::Path::new("unused-mod-storage"),
        Vec::new(),
        community_application::mods::ModLimits {
            max_upload_bytes: 1,
            user_quota_bytes: 1,
            uploads_per_day: 1,
        },
    )
}

/// The real moderation service over the SQLite store. Admins are the given Discord ids.
pub fn moderation_service(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
    admin_discord_ids: Vec<DiscordId>,
) -> Arc<community_application::ModerationService> {
    Arc::new(community_application::ModerationService {
        clock,
        tokens: Arc::new(OsTokenGenerator),
        repo: store.clone(),
        audit: store.clone(),
        comments: store.clone(),
        users: store,
        admin_discord_ids,
    })
}

/// The dev-mode seeder over the SQLite store and the given mod hosting service.
pub fn dev_seeder(
    store: Arc<SqliteStore>,
    clock: Arc<dyn Clock>,
    mods: Arc<community_application::ModHostingService>,
    admin_discord_ids: Vec<DiscordId>,
) -> community_application::dev_seed::DevSeeder {
    community_application::dev_seed::DevSeeder {
        clock: clock.clone(),
        tokens: Arc::new(OsTokenGenerator),
        users: store.clone(),
        mods,
        comments: Arc::new(CommentsService {
            clock: clock.clone(),
            tokens: Arc::new(OsTokenGenerator),
            comments: store.clone(),
        }),
        stars: star_service(store.clone()),
        downloads: Arc::new(community_application::DownloadService {
            clock,
            hasher: Arc::new(Sha256Hasher),
            downloads: store,
        }),
        admin_discord_ids,
    }
}
