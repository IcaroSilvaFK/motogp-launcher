//! SQLite adapter of the download counts (`mod_downloads`).

use community_application::downloads::{DownloadRepository, ModDownloads};
use community_application::ports::PortResult;
use community_application::AppError;
use sqlx::Row;

use crate::SqliteStore;

fn internal(error: sqlx::Error) -> AppError {
    AppError::Internal(format!("database error: {error}"))
}

#[async_trait::async_trait]
impl DownloadRepository for SqliteStore {
    async fn record(&self, mod_id: &str, install_hash: &str, day: i64) -> PortResult<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO mod_downloads (mod_id, install_hash, day) VALUES (?1, ?2, ?3)",
        )
        .bind(mod_id)
        .bind(install_hash)
        .bind(day)
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn totals(&self, since_day: i64) -> PortResult<Vec<ModDownloads>> {
        let rows = sqlx::query(
            "SELECT mod_id, COUNT(*) AS total, COALESCE(SUM(day >= ?1), 0) AS recent
             FROM mod_downloads GROUP BY mod_id ORDER BY mod_id",
        )
        .bind(since_day)
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        Ok(rows
            .iter()
            .map(|row| ModDownloads {
                id: row.get("mod_id"),
                total: row.get::<i64, _>("total").max(0) as u64,
                last_30_days: row.get::<i64, _>("recent").max(0) as u64,
            })
            .collect())
    }

    async fn daily(&self, mod_id: &str, from_day: i64, to_day: i64) -> PortResult<Vec<(i64, u64)>> {
        let rows = sqlx::query(
            "SELECT day, COUNT(*) AS count FROM mod_downloads
             WHERE mod_id = ?1 AND day BETWEEN ?2 AND ?3 GROUP BY day ORDER BY day",
        )
        .bind(mod_id)
        .bind(from_day)
        .bind(to_day)
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        Ok(rows
            .iter()
            .map(|row| (row.get("day"), row.get::<i64, _>("count").max(0) as u64))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    #[tokio::test]
    async fn record_is_idempotent_per_mod_install_and_day() {
        let store = store().await;
        store.record("a", "h1", 10).await.unwrap();
        store.record("a", "h1", 10).await.unwrap();
        store.record("a", "h1", 11).await.unwrap();
        store.record("a", "h2", 11).await.unwrap();
        let totals = store.totals(0).await.unwrap();
        assert_eq!(totals.len(), 1);
        assert_eq!(totals[0].total, 3);
    }

    #[tokio::test]
    async fn totals_are_sorted_and_split_recent_from_old() {
        let store = store().await;
        assert!(store.totals(0).await.unwrap().is_empty());
        store.record("b", "h1", 5).await.unwrap();
        store.record("a", "h1", 5).await.unwrap();
        store.record("a", "h1", 50).await.unwrap();
        let totals = store.totals(40).await.unwrap();
        assert_eq!(
            totals,
            vec![
                ModDownloads {
                    id: "a".into(),
                    total: 2,
                    last_30_days: 1
                },
                ModDownloads {
                    id: "b".into(),
                    total: 1,
                    last_30_days: 0
                },
            ]
        );
    }

    #[tokio::test]
    async fn daily_returns_the_range_only() {
        let store = store().await;
        for (h, d) in [("h1", 9), ("h1", 10), ("h2", 10), ("h1", 12)] {
            store.record("a", h, d).await.unwrap();
        }
        store.record("b", "h1", 10).await.unwrap();
        assert_eq!(store.daily("a", 10, 11).await.unwrap(), vec![(10, 2)]);
        assert!(store.daily("zzz", 0, 99).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn only_the_given_hash_is_stored() {
        let store = store().await;
        store.record("a", "deadbeef", 1).await.unwrap();
        let hash: String = sqlx::query_scalar("SELECT install_hash FROM mod_downloads")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(hash, "deadbeef");
    }
}
