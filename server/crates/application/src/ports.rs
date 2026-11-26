//! Generic ports. Infrastructure implements them; use cases depend on them only.

use chrono::{DateTime, Utc};
use community_domain::{DiscordId, OAuthFlow, OneTimeCode, Session, User, UserId};

use crate::AppError;

pub type PortResult<T> = Result<T, AppError>;

/// Source of the current time (injected so tests control it).
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Cryptographically secure random strings.
pub trait TokenGenerator: Send + Sync {
    /// `byte_len` random bytes, encoded as base64url without padding.
    fn generate(&self, byte_len: usize) -> String;
}

/// One-way hash used to store tokens and codes (lowercase hex SHA-256).
pub trait Hasher: Send + Sync {
    fn hash(&self, input: &str) -> String;
}

/// Reports whether the backing store is reachable (used by `GET /health`).
#[async_trait::async_trait]
pub trait HealthProbe: Send + Sync {
    async fn check(&self) -> PortResult<()>;
}

/// Data needed to create or refresh a user from Discord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserUpsert {
    /// Used only when the Discord id is new; an existing user keeps its id.
    pub new_id: UserId,
    pub discord_id: DiscordId,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar_hash: Option<String>,
    pub now: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait UserRepository: Send + Sync {
    /// Inserts the user, or updates the profile fields of the existing Discord id.
    async fn upsert(&self, user: &UserUpsert) -> PortResult<User>;
    async fn get(&self, id: &UserId) -> PortResult<Option<User>>;
}

#[async_trait::async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, session: &Session) -> PortResult<()>;
    /// Returns the session whatever its state; the caller checks `is_active`.
    async fn find_by_token_hash(&self, token_hash: &str) -> PortResult<Option<Session>>;
    /// Revokes an active session. `true` when one was revoked, `false` when it
    /// was unknown or already revoked.
    async fn revoke(&self, token_hash: &str, now: DateTime<Utc>) -> PortResult<bool>;
}

#[async_trait::async_trait]
pub trait OAuthFlowRepository: Send + Sync {
    async fn save(&self, flow: &OAuthFlow) -> PortResult<()>;
    /// Atomically removes and returns the flow; `None` when unknown or expired.
    async fn take(&self, state: &str, now: DateTime<Utc>) -> PortResult<Option<OAuthFlow>>;
    async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64>;
}

#[async_trait::async_trait]
pub trait OneTimeCodeRepository: Send + Sync {
    async fn save(&self, code: &OneTimeCode) -> PortResult<()>;
    /// Atomically removes and returns the code (single use, consumed on any
    /// attempt); `None` when unknown or expired.
    async fn take(&self, code_hash: &str, now: DateTime<Utc>) -> PortResult<Option<OneTimeCode>>;
    async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64>;
}

/// Discord profile fields the community server keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
}

/// An access token from Discord. Its `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(pub String);

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessToken(<redacted>)")
    }
}

/// Discord OAuth2 (authorization code + PKCE). The adapter knows the client id,
/// the secret and the server's own callback URL (`{PUBLIC_BASE_URL}/auth/discord/callback`).
#[async_trait::async_trait]
pub trait DiscordOAuth: Send + Sync {
    /// URL that sends the user to Discord's consent screen (scope `identify`, S256).
    fn authorize_url(&self, state: &str, code_challenge: &str) -> String;
    /// Trades the authorization code for an access token.
    /// Transport and non-2xx failures map to `AppError::DiscordUnavailable`.
    async fn exchange_code(&self, code: &str, code_verifier: &str) -> PortResult<AccessToken>;
    async fn fetch_user(&self, access_token: &AccessToken) -> PortResult<DiscordUser>;
}
