//! SQLite adapters of the moderation ports. Timestamps are milliseconds, like comments.

use chrono::{DateTime, Utc};
use community_application::comments::CommentWithAuthor;
use community_application::moderation::{
    AuditLog, BannedUser, HiddenComment, ModerationRepository,
};
use community_application::ports::PortResult;
use community_domain::comments::CommentId;
use community_domain::moderation::{AuditEntry, Ban, ModerationAction, Report, ReportReason};
use community_domain::{DiscordId, User, UserId};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::sqlite_comments::{author_from_row, corrupt, from_ms, from_row, internal, COLUMNS};
use crate::SqliteStore;

fn to_ms(time: DateTime<Utc>) -> i64 {
    time.timestamp_millis()
}

/// A comment (with its author) plus the report columns, all prefixed `r_`.
const REPORT_SELECT: &str = "SELECT r.id AS r_id, r.reason AS r_reason, r.details AS r_details,
        r.created_at AS r_created_at, r.reporter_id AS r_reporter_id, {COLUMNS}
    FROM comment_reports r
    JOIN comments c ON c.id = r.comment_id
    JOIN users u ON u.id = c.user_id
    WHERE r.resolved_at IS NULL AND c.hidden_at IS NULL
    ORDER BY r.created_at, r.rowid";

fn report_from_row(row: &SqliteRow) -> PortResult<(Report, CommentWithAuthor)> {
    let reason: String = row.get("r_reason");
    let comment = from_row(row)?;
    Ok((
        Report {
            id: row.get("r_id"),
            comment_id: comment.comment.id.clone(),
            reporter_id: UserId(row.get("r_reporter_id")),
            reason: ReportReason::parse(&reason).ok_or_else(|| corrupt("report reason"))?,
            details: row.get("r_details"),
            created_at: from_ms(row.get("r_created_at")),
        },
        comment,
    ))
}

#[async_trait::async_trait]
impl ModerationRepository for SqliteStore {
    async fn insert_report(&self, report: &Report) -> PortResult<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO comment_reports
                 (id, comment_id, reporter_id, reason, details, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&report.id)
        .bind(report.comment_id.as_str())
        .bind(report.reporter_id.as_str())
        .bind(report.reason.as_str())
        .bind(&report.details)
        .bind(to_ms(report.created_at))
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn open_reports(&self) -> PortResult<Vec<(Report, CommentWithAuthor)>> {
        let sql = REPORT_SELECT.replace("{COLUMNS}", COLUMNS);
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool())
            .await
            .map_err(internal)?;
        rows.iter().map(report_from_row).collect()
    }

    async fn dismiss_report(
        &self,
        id: &str,
        admin: &UserId,
        now: DateTime<Utc>,
    ) -> PortResult<Option<CommentId>> {
        let comment: Option<String> = sqlx::query_scalar(
            "UPDATE comment_reports
             SET resolved_at = ?2, resolved_by = ?3, resolution = 'dismissed'
             WHERE id = ?1 AND resolved_at IS NULL
             RETURNING comment_id",
        )
        .bind(id)
        .bind(to_ms(now))
        .bind(admin.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(internal)?;
        Ok(comment.map(CommentId))
    }

    async fn hide_comment(
        &self,
        id: &CommentId,
        admin: &UserId,
        reason: &str,
        now: DateTime<Utc>,
    ) -> PortResult<bool> {
        let mut tx = self.pool().begin().await.map_err(internal)?;
        let hidden = sqlx::query(
            "UPDATE comments SET hidden_at = ?2, hidden_by = ?3, hidden_reason = ?4
             WHERE id = ?1 AND hidden_at IS NULL",
        )
        .bind(id.as_str())
        .bind(to_ms(now))
        .bind(admin.as_str())
        .bind(reason)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        if hidden.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query(
            "UPDATE comment_reports
             SET resolved_at = ?2, resolved_by = ?3, resolution = 'hidden'
             WHERE comment_id = ?1 AND resolved_at IS NULL",
        )
        .bind(id.as_str())
        .bind(to_ms(now))
        .bind(admin.as_str())
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        tx.commit().await.map_err(internal)?;
        Ok(true)
    }

    async fn restore_comment(&self, id: &CommentId) -> PortResult<bool> {
        sqlx::query(
            "UPDATE comments SET hidden_at = NULL, hidden_by = NULL, hidden_reason = NULL
             WHERE id = ?1 AND hidden_at IS NOT NULL",
        )
        .bind(id.as_str())
        .execute(self.pool())
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(internal)
    }

    async fn hidden_comments(&self) -> PortResult<Vec<HiddenComment>> {
        let rows = sqlx::query(&format!(
            "SELECT {COLUMNS}, c.hidden_at, c.hidden_by, c.hidden_reason
             FROM comments c JOIN users u ON u.id = c.user_id
             WHERE c.hidden_at IS NOT NULL
             ORDER BY c.hidden_at DESC, c.rowid DESC"
        ))
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        rows.iter()
            .map(|row| {
                Ok(HiddenComment {
                    comment: from_row(row)?,
                    hidden_at: from_ms(row.get("hidden_at")),
                    hidden_by: row.get::<Option<String>, _>("hidden_by").map(UserId),
                    reason: row.get("hidden_reason"),
                })
            })
            .collect()
    }

    async fn ban(&self, ban: &Ban) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO user_bans (user_id, banned_by, reason, banned_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id) DO UPDATE SET
                 banned_by = excluded.banned_by,
                 reason = excluded.reason,
                 banned_at = excluded.banned_at",
        )
        .bind(ban.user_id.as_str())
        .bind(ban.banned_by.as_ref().map(UserId::as_str))
        .bind(&ban.reason)
        .bind(to_ms(ban.banned_at))
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(internal)
    }

    async fn unban(&self, user: &UserId) -> PortResult<bool> {
        sqlx::query("DELETE FROM user_bans WHERE user_id = ?1")
            .bind(user.as_str())
            .execute(self.pool())
            .await
            .map(|result| result.rows_affected() > 0)
            .map_err(internal)
    }

    async fn is_banned(&self, user: &UserId) -> PortResult<bool> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM user_bans WHERE user_id = ?1)")
            .bind(user.as_str())
            .fetch_one(self.pool())
            .await
            .map_err(internal)
    }

    async fn bans(&self) -> PortResult<Vec<BannedUser>> {
        let rows = sqlx::query(
            "SELECT b.user_id, b.banned_by, b.reason, b.banned_at,
                    u.id AS u_id, u.discord_id AS u_discord_id, u.username AS u_username,
                    u.global_name AS u_global_name, u.avatar_hash AS u_avatar_hash,
                    u.created_at AS u_created_at
             FROM user_bans b JOIN users u ON u.id = b.user_id
             ORDER BY b.banned_at DESC, b.rowid DESC",
        )
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        rows.iter()
            .map(|row| {
                Ok(BannedUser {
                    user: author_from_row(row)?,
                    ban: Ban {
                        user_id: UserId(row.get("user_id")),
                        banned_by: row.get::<Option<String>, _>("banned_by").map(UserId),
                        reason: row.get("reason"),
                        banned_at: from_ms(row.get("banned_at")),
                    },
                })
            })
            .collect()
    }

    async fn find_user_by_discord_id(&self, id: &DiscordId) -> PortResult<Option<User>> {
        let row = sqlx::query(
            "SELECT id AS u_id, discord_id AS u_discord_id, username AS u_username,
                    global_name AS u_global_name, avatar_hash AS u_avatar_hash,
                    created_at AS u_created_at
             FROM users WHERE discord_id = ?1",
        )
        .bind(id.as_str())
        .fetch_optional(self.pool())
        .await
        .map_err(internal)?;
        row.as_ref().map(author_from_row).transpose()
    }

    async fn recent_audit(&self, limit: u32) -> PortResult<Vec<AuditEntry>> {
        let rows = sqlx::query(
            "SELECT a.id, a.admin_id, a.action, a.target, a.reason, a.created_at,
                    COALESCE(NULLIF(u.global_name, ''), u.username) AS admin_name
             FROM moderation_actions a LEFT JOIN users u ON u.id = a.admin_id
             ORDER BY a.created_at DESC, a.rowid DESC
             LIMIT ?1",
        )
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        rows.iter()
            .map(|row| {
                let action: String = row.get("action");
                Ok(AuditEntry {
                    id: row.get("id"),
                    admin_id: row.get::<Option<String>, _>("admin_id").map(UserId),
                    admin_name: row.get("admin_name"),
                    action: ModerationAction::parse(&action)
                        .ok_or_else(|| corrupt("moderation action"))?,
                    target: row.get("target"),
                    reason: row.get("reason"),
                    created_at: from_ms(row.get("created_at")),
                })
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl AuditLog for SqliteStore {
    async fn record(&self, entry: &AuditEntry) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO moderation_actions (id, admin_id, action, target, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&entry.id)
        .bind(entry.admin_id.as_ref().map(UserId::as_str))
        .bind(entry.action.as_str())
        .bind(&entry.target)
        .bind(&entry.reason)
        .bind(to_ms(entry.created_at))
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(internal)
    }
}

#[cfg(test)]
mod tests {
    use community_application::comments::CommentRepository;
    use community_application::ports::{UserRepository, UserUpsert};
    use community_domain::comments::{Comment, CommentBody, ModId};

    use super::*;

    async fn store() -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    async fn add_user(store: &SqliteStore, id: &str, discord: &str) -> User {
        store
            .upsert(&UserUpsert {
                new_id: UserId(id.into()),
                discord_id: DiscordId::parse(discord).unwrap(),
                username: format!("{id}-name"),
                global_name: (id != "u3").then(|| format!("{id} display")),
                avatar_hash: None,
                now: DateTime::from_timestamp(1_000, 0).unwrap(),
            })
            .await
            .unwrap()
    }

    async fn add_comment(store: &SqliteStore, id: &str, user: &User, ms: i64) -> Comment {
        let comment = Comment {
            id: CommentId(id.into()),
            mod_id: ModId::parse("mod-a").unwrap(),
            user_id: user.id.clone(),
            body: CommentBody::parse(&format!("body {id}")).unwrap(),
            created_at: from_ms(ms),
            edited_at: None,
        };
        CommentRepository::insert(store, &comment).await.unwrap();
        comment
    }

    fn mod_a() -> ModId {
        ModId::parse("mod-a").unwrap()
    }

    fn report(id: &str, comment: &str, reporter: &str, reason: ReportReason, ms: i64) -> Report {
        Report {
            id: id.into(),
            comment_id: CommentId(comment.into()),
            reporter_id: UserId(reporter.into()),
            reason,
            details: Some("details".into()),
            created_at: from_ms(ms),
        }
    }

    fn cid(id: &str) -> CommentId {
        CommentId(id.into())
    }

    fn uid(id: &str) -> UserId {
        UserId(id.into())
    }

    async fn visible_ids(store: &SqliteStore) -> Vec<String> {
        CommentRepository::page(store, &mod_a(), None, 50)
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.comment.id.0)
            .collect()
    }

    #[tokio::test]
    async fn reports_are_unique_per_user_and_comment() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_user(&store, "u2", "22").await;
        add_comment(&store, "c1", &author, 1_000).await;

        store
            .insert_report(&report("r1", "c1", "u2", ReportReason::Spam, 2_000))
            .await
            .unwrap();
        store
            .insert_report(&report("r2", "c1", "u2", ReportReason::Abuse, 3_000))
            .await
            .unwrap();

        let open = store.open_reports().await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].0.id, "r1");
        assert_eq!(open[0].0.reason, ReportReason::Spam);
        assert_eq!(open[0].0.details.as_deref(), Some("details"));
        assert_eq!(open[0].1.comment.id.as_str(), "c1");
        assert_eq!(open[0].1.author.id, author.id);
    }

    #[tokio::test]
    async fn open_reports_are_oldest_first_and_skip_resolved_ones() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_user(&store, "u2", "22").await;
        add_user(&store, "u3", "33").await;
        add_comment(&store, "c1", &author, 1_000).await;
        add_comment(&store, "c2", &author, 1_001).await;
        store
            .insert_report(&report("r2", "c2", "u2", ReportReason::Other, 5_000))
            .await
            .unwrap();
        store
            .insert_report(&report("r1", "c1", "u2", ReportReason::Spam, 4_000))
            .await
            .unwrap();
        store
            .insert_report(&report("r3", "c1", "u3", ReportReason::Spam, 6_000))
            .await
            .unwrap();

        let ids: Vec<_> = store
            .open_reports()
            .await
            .unwrap()
            .into_iter()
            .map(|(r, _)| r.id)
            .collect();
        assert_eq!(ids, ["r1", "r2", "r3"]);

        let comment = store
            .dismiss_report("r1", &uid("u1"), from_ms(7_000))
            .await
            .unwrap();
        assert_eq!(comment, Some(cid("c1")));
        assert_eq!(
            store
                .dismiss_report("r1", &uid("u1"), from_ms(8_000))
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .dismiss_report("nope", &uid("u1"), from_ms(8_000))
                .await
                .unwrap(),
            None
        );
        assert_eq!(store.open_reports().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn hiding_removes_the_comment_from_reads_and_resolves_its_reports() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_user(&store, "u2", "22").await;
        add_comment(&store, "c1", &author, 1_000).await;
        add_comment(&store, "c2", &author, 2_000).await;
        store
            .insert_report(&report("r1", "c1", "u2", ReportReason::Spam, 3_000))
            .await
            .unwrap();

        assert!(store
            .hide_comment(&cid("c1"), &uid("u2"), "advertising", from_ms(4_000))
            .await
            .unwrap());
        // Already hidden and unknown comments change nothing.
        assert!(!store
            .hide_comment(&cid("c1"), &uid("u2"), "again", from_ms(5_000))
            .await
            .unwrap());
        assert!(!store
            .hide_comment(&cid("nope"), &uid("u2"), "x", from_ms(5_000))
            .await
            .unwrap());

        assert_eq!(visible_ids(&store).await, ["c2"]);
        assert!(CommentRepository::find(&store, &mod_a(), &cid("c1"))
            .await
            .unwrap()
            .is_none());
        assert!(store.open_reports().await.unwrap().is_empty());
        let resolution: String =
            sqlx::query_scalar("SELECT resolution FROM comment_reports WHERE id = 'r1'")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(resolution, "hidden");

        let hidden = store.hidden_comments().await.unwrap();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].comment.comment.id.as_str(), "c1");
        assert_eq!(hidden[0].hidden_at, from_ms(4_000));
        assert_eq!(hidden[0].hidden_by, Some(uid("u2")));
        assert_eq!(hidden[0].reason.as_deref(), Some("advertising"));
    }

    #[tokio::test]
    async fn hidden_comments_cannot_be_edited_or_deleted_by_their_author() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_comment(&store, "c1", &author, 1_000).await;
        store
            .hide_comment(&cid("c1"), &uid("u1"), "x", from_ms(2_000))
            .await
            .unwrap();

        let body = CommentBody::parse("new").unwrap();
        assert!(!CommentRepository::update_body(
            &store,
            &mod_a(),
            &cid("c1"),
            &author.id,
            &body,
            from_ms(3_000)
        )
        .await
        .unwrap());
        assert!(
            !CommentRepository::delete(&store, &mod_a(), &cid("c1"), &author.id)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn restoring_shows_the_comment_again() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_comment(&store, "c1", &author, 1_000).await;
        assert!(!store.restore_comment(&cid("c1")).await.unwrap());
        store
            .hide_comment(&cid("c1"), &uid("u1"), "x", from_ms(2_000))
            .await
            .unwrap();
        assert!(store.restore_comment(&cid("c1")).await.unwrap());
        assert!(!store.restore_comment(&cid("c1")).await.unwrap());
        assert_eq!(visible_ids(&store).await, ["c1"]);
        assert!(store.hidden_comments().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_ban_hides_the_users_comments_until_it_is_lifted() {
        let store = store().await;
        let banned = add_user(&store, "u1", "11").await;
        let other = add_user(&store, "u2", "22").await;
        add_comment(&store, "c1", &banned, 1_000).await;
        add_comment(&store, "c2", &other, 2_000).await;
        add_comment(&store, "c3", &banned, 3_000).await;
        assert!(!store.is_banned(&banned.id).await.unwrap());

        store
            .ban(&Ban {
                user_id: banned.id.clone(),
                banned_by: Some(other.id.clone()),
                reason: "spam".into(),
                banned_at: from_ms(4_000),
            })
            .await
            .unwrap();
        assert!(store.is_banned(&banned.id).await.unwrap());
        assert!(!store.is_banned(&other.id).await.unwrap());
        assert_eq!(visible_ids(&store).await, ["c2"]);

        // Keyset pagination across hidden rows: page size 1 still reaches everything visible.
        let first = CommentRepository::page(&store, &mod_a(), None, 1)
            .await
            .unwrap();
        assert_eq!(first[0].comment.id.as_str(), "c2");

        assert!(store.unban(&banned.id).await.unwrap());
        assert!(!store.unban(&banned.id).await.unwrap());
        assert_eq!(visible_ids(&store).await, ["c1", "c2", "c3"]);
    }

    #[tokio::test]
    async fn comments_hidden_one_by_one_stay_hidden_after_an_unban() {
        let store = store().await;
        let user = add_user(&store, "u1", "11").await;
        add_comment(&store, "c1", &user, 1_000).await;
        add_comment(&store, "c2", &user, 2_000).await;
        store
            .hide_comment(&cid("c1"), &uid("u1"), "x", from_ms(3_000))
            .await
            .unwrap();
        store
            .ban(&Ban {
                user_id: user.id.clone(),
                banned_by: None,
                reason: "r".into(),
                banned_at: from_ms(4_000),
            })
            .await
            .unwrap();
        store.unban(&user.id).await.unwrap();
        assert_eq!(visible_ids(&store).await, ["c2"]);
    }

    #[tokio::test]
    async fn banning_twice_replaces_the_ban_and_lists_newest_first() {
        let store = store().await;
        let first = add_user(&store, "u1", "11").await;
        let second = add_user(&store, "u2", "22").await;
        for (user, reason, ms) in [
            (&first, "one", 1_000),
            (&second, "two", 2_000),
            (&first, "three", 3_000),
        ] {
            store
                .ban(&Ban {
                    user_id: user.id.clone(),
                    banned_by: Some(second.id.clone()),
                    reason: reason.into(),
                    banned_at: from_ms(ms),
                })
                .await
                .unwrap();
        }
        let bans = store.bans().await.unwrap();
        assert_eq!(bans.len(), 2);
        assert_eq!(bans[0].user, first);
        assert_eq!(bans[0].ban.reason, "three");
        assert_eq!(bans[0].ban.banned_by, Some(second.id.clone()));
        assert_eq!(bans[1].user, second);
    }

    #[tokio::test]
    async fn deleting_a_user_removes_their_ban_and_reports() {
        let store = store().await;
        let author = add_user(&store, "u1", "11").await;
        add_user(&store, "u2", "22").await;
        add_comment(&store, "c1", &author, 1_000).await;
        store
            .insert_report(&report("r1", "c1", "u2", ReportReason::Spam, 2_000))
            .await
            .unwrap();
        store
            .ban(&Ban {
                user_id: uid("u2"),
                banned_by: Some(uid("u1")),
                reason: "r".into(),
                banned_at: from_ms(3_000),
            })
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id = 'u2'")
            .execute(store.pool())
            .await
            .unwrap();
        assert!(store.open_reports().await.unwrap().is_empty());
        assert!(store.bans().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn users_are_found_by_discord_id() {
        let store = store().await;
        let user = add_user(&store, "u1", "11").await;
        let found = store
            .find_user_by_discord_id(&DiscordId::parse("11").unwrap())
            .await
            .unwrap();
        assert_eq!(found, Some(user));
        assert_eq!(
            store
                .find_user_by_discord_id(&DiscordId::parse("12").unwrap())
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn the_audit_log_lists_newest_first_with_the_admin_name() {
        let store = store().await;
        add_user(&store, "u1", "11").await;
        add_user(&store, "u3", "33").await; // no global name: falls back to the username
        let entry = |id: &str, admin: Option<&str>, action, ms| AuditEntry {
            id: id.into(),
            admin_id: admin.map(uid),
            admin_name: None,
            action,
            target: "t".into(),
            reason: Some("why".into()),
            created_at: from_ms(ms),
        };
        store
            .record(&entry(
                "a1",
                Some("u1"),
                ModerationAction::HideComment,
                1_000,
            ))
            .await
            .unwrap();
        store
            .record(&entry("a2", Some("u3"), ModerationAction::BanUser, 2_000))
            .await
            .unwrap();
        store
            .record(&entry("a3", None, ModerationAction::RejectVersion, 3_000))
            .await
            .unwrap();

        let rows = store.recent_audit(2).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "a3");
        assert_eq!(rows[0].admin_name, None);
        assert_eq!(rows[0].admin_id, None);
        assert_eq!(rows[1].id, "a2");
        assert_eq!(rows[1].admin_name.as_deref(), Some("u3-name"));
        assert_eq!(rows[1].created_at, from_ms(2_000));
        let all = store.recent_audit(10).await.unwrap();
        assert_eq!(all[2].admin_name.as_deref(), Some("u1 display"));
        assert_eq!(all[2].reason.as_deref(), Some("why"));
    }

    #[tokio::test]
    async fn a_deleted_admin_keeps_the_audit_row() {
        let store = store().await;
        add_user(&store, "u1", "11").await;
        store
            .record(&AuditEntry {
                id: "a1".into(),
                admin_id: Some(uid("u1")),
                admin_name: None,
                action: ModerationAction::UnbanUser,
                target: "x".into(),
                reason: None,
                created_at: from_ms(1_000),
            })
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id = 'u1'")
            .execute(store.pool())
            .await
            .unwrap();
        let rows = store.recent_audit(5).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].admin_id, None);
    }
}
