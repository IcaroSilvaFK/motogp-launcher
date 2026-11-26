//! Helpers shared by unit and integration tests (no network, no real time).

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};
use community_application::ports::{AccessToken, Clock, DiscordOAuth, DiscordUser, PortResult};
use community_application::{AppError, AuthService};
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
