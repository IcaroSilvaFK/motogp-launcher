//! SQLite adapter of the comment repository. Timestamps are stored in
//! milliseconds so comments posted within the same second keep their order.

use chrono::{DateTime, Utc};
use community_application::comments::{CommentRepository, CommentWithAuthor};
use community_application::ports::PortResult;
use community_application::AppError;
use community_domain::comments::{Comment, CommentBody, CommentId, ModId, PageCursor};
use community_domain::{DiscordId, User, UserId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::SqliteStore;

fn internal(error: sqlx::Error) -> AppError {
    AppError::Internal(format!("database error: {error}"))
}

fn corrupt(what: &str) -> AppError {
    AppError::Internal(format!("stored {what} is invalid"))
}

fn from_ms(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

const SELECT: &str = "SELECT c.id, c.mod_id, c.body, c.created_at, c.edited_at,
        u.id AS u_id, u.discord_id AS u_discord_id, u.username AS u_username,
        u.global_name AS u_global_name, u.avatar_hash AS u_avatar_hash,
        u.created_at AS u_created_at
    FROM comments c JOIN users u ON u.id = c.user_id";

fn from_row(row: &SqliteRow) -> PortResult<CommentWithAuthor> {
    let user_id = UserId(row.get("u_id"));
    let discord_id: String = row.get("u_discord_id");
    let mod_id: String = row.get("mod_id");
    let body: String = row.get("body");
    Ok(CommentWithAuthor {
        comment: Comment {
            id: CommentId(row.get("id")),
            mod_id: ModId::parse(&mod_id).ok_or_else(|| corrupt("mod id"))?,
            user_id: user_id.clone(),
            body: CommentBody::parse(&body).map_err(|_| corrupt("comment body"))?,
            created_at: from_ms(row.get("created_at")),
            edited_at: row.get::<Option<i64>, _>("edited_at").map(from_ms),
        },
        author: User {
            id: user_id,
            discord_id: DiscordId::parse(&discord_id).ok_or_else(|| corrupt("discord id"))?,
            username: row.get("u_username"),
            global_name: row.get("u_global_name"),
            avatar_hash: row.get("u_avatar_hash"),
            created_at: DateTime::from_timestamp(row.get("u_created_at"), 0).unwrap_or_default(),
        },
    })
}

#[async_trait::async_trait]
impl CommentRepository for SqliteStore {
    async fn insert(&self, comment: &Comment) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO comments (id, mod_id, user_id, body, created_at, edited_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(comment.id.as_str())
        .bind(comment.mod_id.as_str())
        .bind(comment.user_id.as_str())
        .bind(comment.body.as_str())
        .bind(comment.created_at.timestamp_millis())
        .bind(comment.edited_at.map(|t| t.timestamp_millis()))
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn page(
        &self,
        mod_id: &ModId,
        after: Option<&PageCursor>,
        limit: u32,
    ) -> PortResult<Vec<CommentWithAuthor>> {
        let rows = sqlx::query(&format!(
            "{SELECT}
             WHERE c.mod_id = ?1
               AND (?2 IS NULL OR c.created_at > ?2 OR (c.created_at = ?2 AND c.id > ?3))
             ORDER BY c.created_at, c.id
             LIMIT ?4"
        ))
        .bind(mod_id.as_str())
        .bind(after.map(|c| c.created_at_ms))
        .bind(after.map(|c| c.id.as_str()))
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        rows.iter().map(from_row).collect()
    }

    async fn find(&self, mod_id: &ModId, id: &CommentId) -> PortResult<Option<CommentWithAuthor>> {
        let row = sqlx::query(&format!("{SELECT} WHERE c.mod_id = ?1 AND c.id = ?2"))
            .bind(mod_id.as_str())
            .bind(id.as_str())
            .fetch_optional(self.pool())
            .await
            .map_err(internal)?;
        row.as_ref().map(from_row).transpose()
    }

    async fn update_body(
        &self,
        mod_id: &ModId,
        id: &CommentId,
        author: &UserId,
        body: &CommentBody,
        edited_at: DateTime<Utc>,
    ) -> PortResult<bool> {
        let result = sqlx::query(
            "UPDATE comments SET body = ?4, edited_at = ?5
             WHERE id = ?1 AND mod_id = ?2 AND user_id = ?3",
        )
        .bind(id.as_str())
        .bind(mod_id.as_str())
        .bind(author.as_str())
        .bind(body.as_str())
        .bind(edited_at.timestamp_millis())
        .execute(self.pool())
        .await
        .map_err(internal)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete(&self, mod_id: &ModId, id: &CommentId, author: &UserId) -> PortResult<bool> {
        let result =
            sqlx::query("DELETE FROM comments WHERE id = ?1 AND mod_id = ?2 AND user_id = ?3")
                .bind(id.as_str())
                .bind(mod_id.as_str())
                .bind(author.as_str())
                .execute(self.pool())
                .await
                .map_err(internal)?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use community_application::ports::{UserRepository, UserUpsert};

    use super::*;

    async fn store() -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    async fn add_user(store: &SqliteStore, id: &str, discord: &str, name: Option<&str>) -> User {
        store
            .upsert(&UserUpsert {
                new_id: UserId(id.into()),
                discord_id: DiscordId::parse(discord).unwrap(),
                username: format!("{id}-name"),
                global_name: name.map(str::to_owned),
                avatar_hash: Some("hash".into()),
                now: DateTime::from_timestamp(1_000, 0).unwrap(),
            })
            .await
            .unwrap()
    }

    fn comment(id: &str, mod_id: &str, user: &User, body: &str, ms: i64) -> Comment {
        Comment {
            id: CommentId(id.into()),
            mod_id: ModId::parse(mod_id).unwrap(),
            user_id: user.id.clone(),
            body: CommentBody::parse(body).unwrap(),
            created_at: from_ms(ms),
            edited_at: None,
        }
    }

    fn mod_id(value: &str) -> ModId {
        ModId::parse(value).unwrap()
    }

    #[tokio::test]
    async fn insert_then_find_returns_the_comment_with_its_author() {
        let store = store().await;
        let user = add_user(&store, "u1", "11", Some("Rider One")).await;
        let original = comment("c1", "mod-a", &user, "Great livery", 1_700_000_000_123);
        store.insert(&original).await.unwrap();

        let found = store
            .find(&mod_id("mod-a"), &CommentId("c1".into()))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.comment, original);
        assert_eq!(found.author, user);
        assert_eq!(
            found.comment.created_at.timestamp_millis(),
            1_700_000_000_123
        );
    }

    #[tokio::test]
    async fn find_is_scoped_to_the_mod() {
        let store = store().await;
        let user = add_user(&store, "u1", "11", None).await;
        store
            .insert(&comment("c1", "mod-a", &user, "x", 1))
            .await
            .unwrap();
        assert!(store
            .find(&mod_id("mod-b"), &CommentId("c1".into()))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn page_orders_by_time_then_id_and_resumes_after_the_cursor() {
        let store = store().await;
        let user = add_user(&store, "u1", "11", None).await;
        // Inserted out of order; "b" and "c" share a timestamp (id breaks the tie).
        for (id, ms) in [("c", 20), ("a", 10), ("d", 30), ("b", 20)] {
            store
                .insert(&comment(id, "mod-a", &user, id, ms))
                .await
                .unwrap();
        }
        store
            .insert(&comment("z", "mod-b", &user, "other", 5))
            .await
            .unwrap();

        let ids = |rows: &[CommentWithAuthor]| {
            rows.iter()
                .map(|r| r.comment.id.0.clone())
                .collect::<Vec<_>>()
        };
        let all = store.page(&mod_id("mod-a"), None, 10).await.unwrap();
        assert_eq!(ids(&all), ["a", "b", "c", "d"]);

        let first = store.page(&mod_id("mod-a"), None, 2).await.unwrap();
        assert_eq!(ids(&first), ["a", "b"]);

        let cursor = PageCursor {
            created_at_ms: 20,
            id: "b".into(),
        };
        let rest = store
            .page(&mod_id("mod-a"), Some(&cursor), 10)
            .await
            .unwrap();
        assert_eq!(ids(&rest), ["c", "d"]);
    }

    #[tokio::test]
    async fn update_body_only_works_for_the_author_under_the_right_mod() {
        let store = store().await;
        let author = add_user(&store, "u1", "11", None).await;
        let other = add_user(&store, "u2", "22", None).await;
        store
            .insert(&comment("c1", "mod-a", &author, "old", 1))
            .await
            .unwrap();
        let id = CommentId("c1".into());
        let body = CommentBody::parse("new").unwrap();
        let at = from_ms(5_000);

        assert!(!store
            .update_body(&mod_id("mod-a"), &id, &other.id, &body, at)
            .await
            .unwrap());
        assert!(!store
            .update_body(&mod_id("mod-b"), &id, &author.id, &body, at)
            .await
            .unwrap());
        assert!(store
            .update_body(&mod_id("mod-a"), &id, &author.id, &body, at)
            .await
            .unwrap());

        let found = store.find(&mod_id("mod-a"), &id).await.unwrap().unwrap();
        assert_eq!(found.comment.body.as_str(), "new");
        assert_eq!(found.comment.edited_at, Some(at));
    }

    #[tokio::test]
    async fn delete_only_works_for_the_author_under_the_right_mod() {
        let store = store().await;
        let author = add_user(&store, "u1", "11", None).await;
        let other = add_user(&store, "u2", "22", None).await;
        store
            .insert(&comment("c1", "mod-a", &author, "x", 1))
            .await
            .unwrap();
        let id = CommentId("c1".into());

        assert!(!store
            .delete(&mod_id("mod-a"), &id, &other.id)
            .await
            .unwrap());
        assert!(!store
            .delete(&mod_id("mod-b"), &id, &author.id)
            .await
            .unwrap());
        assert!(store
            .delete(&mod_id("mod-a"), &id, &author.id)
            .await
            .unwrap());
        assert!(store.find(&mod_id("mod-a"), &id).await.unwrap().is_none());
        assert!(!store
            .delete(&mod_id("mod-a"), &id, &author.id)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn raw_html_is_stored_verbatim() {
        let store = store().await;
        let user = add_user(&store, "u1", "11", None).await;
        let html = "<img src=x onerror=alert(1)>";
        store
            .insert(&comment("c1", "mod-a", &user, html, 1))
            .await
            .unwrap();
        let stored: String = sqlx::query_scalar("SELECT body FROM comments WHERE id = 'c1'")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(stored, html);
    }

    #[tokio::test]
    async fn deleting_a_user_removes_their_comments() {
        let store = store().await;
        let user = add_user(&store, "u1", "11", None).await;
        store
            .insert(&comment("c1", "mod-a", &user, "x", 1))
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id = 'u1'")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store
            .page(&mod_id("mod-a"), None, 10)
            .await
            .unwrap()
            .is_empty());
    }
}
