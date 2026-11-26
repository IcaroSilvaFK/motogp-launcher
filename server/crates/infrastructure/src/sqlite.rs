//! SQLite adapters for the repository ports (WAL, foreign keys, embedded migrations).

use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use community_application::ports::{
    HealthProbe, OAuthFlowRepository, OneTimeCodeRepository, PortResult, SessionRepository,
    UserRepository, UserUpsert,
};
use community_application::AppError;
use community_domain::{DiscordId, FlowPurpose, OAuthFlow, OneTimeCode, Session, User, UserId};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

fn internal(error: sqlx::Error) -> AppError {
    AppError::Internal(format!("database error: {error}"))
}

fn to_ts(time: DateTime<Utc>) -> i64 {
    time.timestamp()
}

fn from_ts(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).unwrap_or_default()
}

fn user_from_row(row: &SqliteRow) -> PortResult<User> {
    let discord_id: String = row.get("discord_id");
    Ok(User {
        id: UserId(row.get("id")),
        discord_id: DiscordId::parse(&discord_id)
            .ok_or_else(|| AppError::Internal("stored discord id is invalid".into()))?,
        username: row.get("username"),
        global_name: row.get("global_name"),
        avatar_hash: row.get("avatar_hash"),
        created_at: from_ts(row.get("created_at")),
    })
}

impl SqliteStore {
    /// Opens the database (creating the file when missing) with WAL and
    /// foreign keys on. Does not migrate; call [`SqliteStore::migrate`].
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    /// In-memory database for tests (one connection, so all queries share it).
    pub async fn connect_in_memory() -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        sqlx::migrate!("../../migrations").run(&self.pool).await
    }

    /// The underlying pool (for adapters that need raw access, e.g. tests).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl HealthProbe for SqliteStore {
    async fn check(&self) -> PortResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(internal)
    }
}

#[async_trait::async_trait]
impl UserRepository for SqliteStore {
    async fn upsert(&self, user: &UserUpsert) -> PortResult<User> {
        let now = to_ts(user.now);
        let row = sqlx::query(
            "INSERT INTO users (id, discord_id, username, global_name, avatar_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(discord_id) DO UPDATE SET
                 username = excluded.username,
                 global_name = excluded.global_name,
                 avatar_hash = excluded.avatar_hash,
                 updated_at = excluded.updated_at
             RETURNING id, discord_id, username, global_name, avatar_hash, created_at",
        )
        .bind(user.new_id.as_str())
        .bind(user.discord_id.as_str())
        .bind(&user.username)
        .bind(&user.global_name)
        .bind(&user.avatar_hash)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(internal)?;
        user_from_row(&row)
    }

    async fn get(&self, id: &UserId) -> PortResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id, discord_id, username, global_name, avatar_hash, created_at
             FROM users WHERE id = ?1",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        row.as_ref().map(user_from_row).transpose()
    }
}

#[async_trait::async_trait]
impl SessionRepository for SqliteStore {
    async fn create(&self, session: &Session) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&session.id)
        .bind(session.user_id.as_str())
        .bind(&session.token_hash)
        .bind(to_ts(session.created_at))
        .bind(to_ts(session.expires_at))
        .bind(session.revoked_at.map(to_ts))
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn find_by_token_hash(&self, token_hash: &str) -> PortResult<Option<Session>> {
        let row = sqlx::query(
            "SELECT id, user_id, token_hash, created_at, expires_at, revoked_at
             FROM sessions WHERE token_hash = ?1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        Ok(row.map(|row| Session {
            id: row.get("id"),
            user_id: UserId(row.get("user_id")),
            token_hash: row.get("token_hash"),
            created_at: from_ts(row.get("created_at")),
            expires_at: from_ts(row.get("expires_at")),
            revoked_at: row.get::<Option<i64>, _>("revoked_at").map(from_ts),
        }))
    }

    async fn revoke(&self, token_hash: &str, now: DateTime<Utc>) -> PortResult<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET revoked_at = ?2 WHERE token_hash = ?1 AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .bind(to_ts(now))
        .execute(&self.pool)
        .await
        .map_err(internal)?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait::async_trait]
impl OAuthFlowRepository for SqliteStore {
    async fn save(&self, flow: &OAuthFlow) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO oauth_flows
                 (state, discord_verifier, redirect, app_state, app_challenge, expires_at, purpose)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&flow.state)
        .bind(&flow.discord_verifier)
        .bind(&flow.redirect)
        .bind(&flow.app_state)
        .bind(&flow.app_challenge)
        .bind(to_ts(flow.expires_at))
        .bind(flow.purpose.as_str())
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn take(&self, state: &str, now: DateTime<Utc>) -> PortResult<Option<OAuthFlow>> {
        let row = sqlx::query(
            "DELETE FROM oauth_flows WHERE state = ?1
             RETURNING state, discord_verifier, redirect, app_state, app_challenge, expires_at,
                       purpose",
        )
        .bind(state)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        let Some(row) = row else { return Ok(None) };
        let purpose: String = row.get("purpose");
        let purpose = FlowPurpose::parse(&purpose)
            .ok_or_else(|| AppError::Internal("stored flow purpose is invalid".into()))?;
        Ok(Some(row)
            .map(|row| OAuthFlow {
                purpose,
                state: row.get("state"),
                discord_verifier: row.get("discord_verifier"),
                redirect: row.get("redirect"),
                app_state: row.get("app_state"),
                app_challenge: row.get("app_challenge"),
                expires_at: from_ts(row.get("expires_at")),
            })
            .filter(|flow| flow.expires_at > now))
    }

    async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64> {
        sqlx::query("DELETE FROM oauth_flows WHERE expires_at <= ?1")
            .bind(to_ts(now))
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(internal)
    }
}

#[async_trait::async_trait]
impl OneTimeCodeRepository for SqliteStore {
    async fn save(&self, code: &OneTimeCode) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO one_time_codes (code_hash, user_id, app_challenge, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&code.code_hash)
        .bind(code.user_id.as_str())
        .bind(&code.app_challenge)
        .bind(to_ts(code.expires_at))
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn take(&self, code_hash: &str, now: DateTime<Utc>) -> PortResult<Option<OneTimeCode>> {
        let row = sqlx::query(
            "DELETE FROM one_time_codes WHERE code_hash = ?1
             RETURNING code_hash, user_id, app_challenge, expires_at",
        )
        .bind(code_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(internal)?;
        Ok(row
            .map(|row| OneTimeCode {
                code_hash: row.get("code_hash"),
                user_id: UserId(row.get("user_id")),
                app_challenge: row.get("app_challenge"),
                expires_at: from_ts(row.get("expires_at")),
            })
            .filter(|code| code.expires_at > now))
    }

    async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64> {
        sqlx::query("DELETE FROM one_time_codes WHERE expires_at <= ?1")
            .bind(to_ts(now))
            .execute(&self.pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as Span;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    async fn store() -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    fn upsert(id: &str, discord: &str, name: &str, now: DateTime<Utc>) -> UserUpsert {
        UserUpsert {
            new_id: UserId(id.into()),
            discord_id: DiscordId::parse(discord).unwrap(),
            username: name.into(),
            global_name: Some(format!("{name} global")),
            avatar_hash: None,
            now,
        }
    }

    #[tokio::test]
    async fn health_check_succeeds_on_a_migrated_database() {
        store().await.check().await.unwrap();
    }

    #[tokio::test]
    async fn upsert_keeps_the_id_and_refreshes_the_profile() {
        let store = store().await;
        let first = store
            .upsert(&upsert("u1", "100", "old", at(10)))
            .await
            .unwrap();
        assert_eq!(first.id.as_str(), "u1");
        let second = store
            .upsert(&upsert("u2", "100", "new", at(20)))
            .await
            .unwrap();
        assert_eq!(second.id.as_str(), "u1");
        assert_eq!(second.username, "new");
        assert_eq!(second.created_at, at(10));
        let fetched = store.get(&UserId("u1".into())).await.unwrap().unwrap();
        assert_eq!(fetched, second);
        assert!(store.get(&UserId("nope".into())).await.unwrap().is_none());
    }

    fn session(hash: &str, created: DateTime<Utc>) -> Session {
        Session {
            id: format!("id-{hash}"),
            user_id: UserId("u1".into()),
            token_hash: hash.into(),
            created_at: created,
            expires_at: Session::expiry_from(created),
            revoked_at: None,
        }
    }

    #[tokio::test]
    async fn sessions_are_created_found_and_revoked_once() {
        let store = store().await;
        store
            .upsert(&upsert("u1", "100", "a", at(0)))
            .await
            .unwrap();
        let created = session("hash1", at(100));
        SessionRepository::create(&store, &created).await.unwrap();

        let found = store.find_by_token_hash("hash1").await.unwrap().unwrap();
        assert_eq!(found, created);
        assert!(store.find_by_token_hash("other").await.unwrap().is_none());

        assert!(store.revoke("hash1", at(200)).await.unwrap());
        assert!(!store.revoke("hash1", at(300)).await.unwrap());
        assert!(!store.revoke("missing", at(300)).await.unwrap());
        let revoked = store.find_by_token_hash("hash1").await.unwrap().unwrap();
        assert_eq!(revoked.revoked_at, Some(at(200)));
        assert!(!revoked.is_active(at(201)));
    }

    #[tokio::test]
    async fn session_token_hash_is_unique_and_requires_a_user() {
        let store = store().await;
        assert!(SessionRepository::create(&store, &session("h", at(0)))
            .await
            .is_err());
        store
            .upsert(&upsert("u1", "100", "a", at(0)))
            .await
            .unwrap();
        SessionRepository::create(&store, &session("h", at(0)))
            .await
            .unwrap();
        let mut duplicate = session("h", at(0));
        duplicate.id = "another".into();
        assert!(SessionRepository::create(&store, &duplicate).await.is_err());
    }

    #[tokio::test]
    async fn oauth_flows_are_single_use_and_expire() {
        let store = store().await;
        let flow = OAuthFlow {
            purpose: FlowPurpose::Admin,
            state: "st".into(),
            discord_verifier: "v".into(),
            redirect: "http://127.0.0.1:5000/callback".into(),
            app_state: "as".into(),
            app_challenge: "ch".into(),
            expires_at: at(100) + Span::minutes(10),
        };
        OAuthFlowRepository::save(&store, &flow).await.unwrap();
        assert_eq!(
            OAuthFlowRepository::take(&store, "st", at(101))
                .await
                .unwrap(),
            Some(flow.clone())
        );
        assert!(OAuthFlowRepository::take(&store, "st", at(101))
            .await
            .unwrap()
            .is_none());

        OAuthFlowRepository::save(&store, &flow).await.unwrap();
        assert!(OAuthFlowRepository::take(&store, "st", flow.expires_at)
            .await
            .unwrap()
            .is_none());

        OAuthFlowRepository::save(&store, &flow).await.unwrap();
        assert_eq!(
            OAuthFlowRepository::delete_expired(&store, flow.expires_at)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn one_time_codes_are_single_use_and_expire() {
        let store = store().await;
        store
            .upsert(&upsert("u1", "100", "a", at(0)))
            .await
            .unwrap();
        let code = OneTimeCode {
            code_hash: "ch".into(),
            user_id: UserId("u1".into()),
            app_challenge: "chal".into(),
            expires_at: at(160),
        };
        OneTimeCodeRepository::save(&store, &code).await.unwrap();
        assert_eq!(
            OneTimeCodeRepository::take(&store, "ch", at(100))
                .await
                .unwrap(),
            Some(code.clone())
        );
        assert!(OneTimeCodeRepository::take(&store, "ch", at(100))
            .await
            .unwrap()
            .is_none());

        OneTimeCodeRepository::save(&store, &code).await.unwrap();
        assert!(OneTimeCodeRepository::take(&store, "ch", at(160))
            .await
            .unwrap()
            .is_none());

        OneTimeCodeRepository::save(&store, &code).await.unwrap();
        assert_eq!(
            OneTimeCodeRepository::delete_expired(&store, at(160))
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn connect_creates_a_wal_database_file() {
        let path = std::env::temp_dir().join(format!("community-test-{}.db", std::process::id()));
        let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
        let store = SqliteStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(fk, 1);
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.to_string_lossy()));
        }
    }
}
