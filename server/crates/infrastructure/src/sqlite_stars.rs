//! SQLite adapter of the star repository (table `mod_stars`).

use community_application::ports::PortResult;
use community_application::{AppError, ModStats, StarRepository};
use community_domain::{ModId, UserId};
use sqlx::Row;

use crate::sqlite::SqliteStore;

fn internal(error: sqlx::Error) -> AppError {
    AppError::Internal(format!("database error: {error}"))
}

#[async_trait::async_trait]
impl StarRepository for SqliteStore {
    async fn star(&self, user: &UserId, mod_id: &ModId) -> PortResult<()> {
        sqlx::query("INSERT OR IGNORE INTO mod_stars (user_id, mod_id) VALUES (?, ?)")
            .bind(user.as_str())
            .bind(mod_id.as_str())
            .execute(self.pool())
            .await
            .map_err(internal)?;
        Ok(())
    }

    async fn unstar(&self, user: &UserId, mod_id: &ModId) -> PortResult<()> {
        sqlx::query("DELETE FROM mod_stars WHERE user_id = ? AND mod_id = ?")
            .bind(user.as_str())
            .bind(mod_id.as_str())
            .execute(self.pool())
            .await
            .map_err(internal)?;
        Ok(())
    }

    async fn count(&self, mod_id: &ModId) -> PortResult<u64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mod_stars WHERE mod_id = ?")
            .bind(mod_id.as_str())
            .fetch_one(self.pool())
            .await
            .map_err(internal)?;
        Ok(u64::try_from(count).unwrap_or(0))
    }

    async fn stats(&self, viewer: Option<&UserId>) -> PortResult<Vec<ModStats>> {
        // With no viewer the bound id matches nothing, and the flag is dropped below.
        let rows = sqlx::query(
            "SELECT mod_id, COUNT(*) AS stars, \
                    COALESCE(SUM(user_id = ?), 0) AS mine \
             FROM mod_stars GROUP BY mod_id ORDER BY mod_id",
        )
        .bind(viewer.map_or("", UserId::as_str))
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        Ok(rows
            .iter()
            .map(|row| {
                let stars: i64 = row.get("stars");
                let mine: i64 = row.get("mine");
                ModStats {
                    id: row.get("mod_id"),
                    stars: u64::try_from(stars).unwrap_or(0),
                    starred_by_me: viewer.map(|_| mine > 0),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use community_application::ports::{UserRepository, UserUpsert};
    use community_domain::DiscordId;

    async fn store_with_users(ids: &[&str]) -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        for (index, id) in ids.iter().enumerate() {
            store
                .upsert(&UserUpsert {
                    new_id: UserId((*id).into()),
                    discord_id: DiscordId::parse(&format!("{}", 100 + index)).unwrap(),
                    username: (*id).into(),
                    global_name: None,
                    avatar_hash: None,
                    now: DateTime::from_timestamp(10, 0).unwrap(),
                })
                .await
                .unwrap();
        }
        store
    }

    fn user(id: &str) -> UserId {
        UserId(id.into())
    }

    fn slug(id: &str) -> ModId {
        ModId::parse(id).unwrap()
    }

    #[tokio::test]
    async fn star_is_idempotent_and_counts_per_user() {
        let store = store_with_users(&["u1", "u2"]).await;
        store.star(&user("u1"), &slug("mod-a")).await.unwrap();
        store.star(&user("u1"), &slug("mod-a")).await.unwrap();
        assert_eq!(store.count(&slug("mod-a")).await.unwrap(), 1);
        store.star(&user("u2"), &slug("mod-a")).await.unwrap();
        assert_eq!(store.count(&slug("mod-a")).await.unwrap(), 2);
        assert_eq!(store.count(&slug("other")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn unstar_removes_and_tolerates_a_missing_star() {
        let store = store_with_users(&["u1", "u2"]).await;
        store.star(&user("u1"), &slug("mod-a")).await.unwrap();
        store.star(&user("u2"), &slug("mod-a")).await.unwrap();
        store.unstar(&user("u1"), &slug("mod-a")).await.unwrap();
        store.unstar(&user("u1"), &slug("mod-a")).await.unwrap();
        assert_eq!(store.count(&slug("mod-a")).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn stats_list_starred_mods_by_id_with_the_viewer_flag() {
        let store = store_with_users(&["u1", "u2"]).await;
        store.star(&user("u1"), &slug("zeta")).await.unwrap();
        store.star(&user("u2"), &slug("zeta")).await.unwrap();
        store.star(&user("u2"), &slug("alpha")).await.unwrap();
        store.star(&user("u1"), &slug("beta")).await.unwrap();
        store.unstar(&user("u1"), &slug("beta")).await.unwrap();

        let anonymous = store.stats(None).await.unwrap();
        let ids: Vec<_> = anonymous.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "zeta"]);
        assert_eq!(anonymous[1].stars, 2);
        assert!(anonymous.iter().all(|m| m.starred_by_me.is_none()));

        let mine = store.stats(Some(&user("u1"))).await.unwrap();
        assert_eq!(mine[0].starred_by_me, Some(false));
        assert_eq!(mine[1].starred_by_me, Some(true));
    }

    #[tokio::test]
    async fn stars_are_removed_with_their_user() {
        let store = store_with_users(&["u1"]).await;
        store.star(&user("u1"), &slug("mod-a")).await.unwrap();
        sqlx::query("DELETE FROM users WHERE id = 'u1'")
            .execute(store.pool())
            .await
            .unwrap();
        assert_eq!(store.count(&slug("mod-a")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn starring_for_an_unknown_user_is_an_internal_error() {
        let store = store_with_users(&[]).await;
        let error = store
            .star(&user("ghost"), &slug("mod-a"))
            .await
            .unwrap_err();
        assert_eq!(error.code(), "internal");
    }
}
