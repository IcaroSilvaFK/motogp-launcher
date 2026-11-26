//! Moderation: reports, hiding comments, bans and the audit log.
//!
//! Every admin method takes the acting `User` and refuses a non-admin with
//! `Forbidden`, even though the HTTP layer already authenticates admins: a
//! missed check there must not expose an action. Every action writes one
//! audit row (who, when, what, why).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use community_domain::comments::{CommentId, ModId};
use community_domain::moderation::{
    parse_report_details, AuditEntry, Ban, ModerationAction, ModerationReason, Report, ReportReason,
};
use community_domain::{DiscordId, User, UserId};

use crate::comments::{CommentRepository, CommentWithAuthor};
use crate::ports::{Clock, PortResult, TokenGenerator, UserRepository};
use crate::AppError;

/// Whether the Discord id of the user is in the configured admin list
/// (shared by mod review and moderation).
pub fn is_admin(admin_discord_ids: &[DiscordId], user: &User) -> bool {
    admin_discord_ids.contains(&user.discord_id)
}

/// A comment hidden by an admin, with who hid it and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenComment {
    pub comment: CommentWithAuthor,
    pub hidden_at: DateTime<Utc>,
    pub hidden_by: Option<UserId>,
    pub reason: Option<String>,
}

/// A banned user with the ban details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BannedUser {
    pub user: User,
    pub ban: Ban,
}

/// A comment with its open reports (the admin queue groups per comment).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportedComment {
    pub comment: CommentWithAuthor,
    /// Open reports of the comment, oldest first (never empty).
    pub reports: Vec<Report>,
}

impl ReportedComment {
    pub fn report_count(&self) -> usize {
        self.reports.len()
    }

    /// Distinct reasons in order of first report.
    pub fn reasons(&self) -> Vec<ReportReason> {
        let mut reasons = Vec::new();
        for report in &self.reports {
            if !reasons.contains(&report.reason) {
                reasons.push(report.reason);
            }
        }
        reasons
    }
}

#[async_trait::async_trait]
pub trait ModerationRepository: Send + Sync {
    /// Stores the report; a second report of the same comment by the same user
    /// changes nothing.
    async fn insert_report(&self, report: &Report) -> PortResult<()>;
    /// Open reports with their comments (hidden comments have none), oldest first.
    async fn open_reports(&self) -> PortResult<Vec<(Report, CommentWithAuthor)>>;
    /// Resolves an open report as dismissed. The comment id, or `None` when
    /// there is no open report with that id.
    async fn dismiss_report(
        &self,
        id: &str,
        admin: &UserId,
        now: DateTime<Utc>,
    ) -> PortResult<Option<CommentId>>;
    /// Hides a visible comment and resolves its open reports as `hidden`.
    /// `false` when the comment is unknown or already hidden.
    async fn hide_comment(
        &self,
        id: &CommentId,
        admin: &UserId,
        reason: &str,
        now: DateTime<Utc>,
    ) -> PortResult<bool>;
    /// `false` when the comment is unknown or not hidden.
    async fn restore_comment(&self, id: &CommentId) -> PortResult<bool>;
    /// Hidden comments, most recently hidden first.
    async fn hidden_comments(&self) -> PortResult<Vec<HiddenComment>>;
    /// Bans the user; banning a banned user replaces the ban.
    async fn ban(&self, ban: &Ban) -> PortResult<()>;
    /// `false` when the user was not banned.
    async fn unban(&self, user: &UserId) -> PortResult<bool>;
    async fn is_banned(&self, user: &UserId) -> PortResult<bool>;
    /// Banned users, most recently banned first.
    async fn bans(&self) -> PortResult<Vec<BannedUser>>;
    async fn find_user_by_discord_id(&self, id: &DiscordId) -> PortResult<Option<User>>;
    /// The newest audit rows first, with the admin's display name.
    async fn recent_audit(&self, limit: u32) -> PortResult<Vec<AuditEntry>>;
}

/// Where audit rows are written (also used by the mod review use cases).
#[async_trait::async_trait]
pub trait AuditLog: Send + Sync {
    async fn record(&self, entry: &AuditEntry) -> PortResult<()>;
}

/// Writes one audit row for an admin action.
pub async fn record_audit(
    audit: &dyn AuditLog,
    clock: &dyn Clock,
    tokens: &dyn TokenGenerator,
    admin: &UserId,
    action: ModerationAction,
    target: String,
    reason: Option<String>,
) -> PortResult<()> {
    audit
        .record(&AuditEntry {
            id: tokens.generate(16),
            admin_id: Some(admin.clone()),
            admin_name: None,
            action,
            target,
            reason,
            created_at: clock.now(),
        })
        .await
}

pub struct ModerationService {
    pub clock: Arc<dyn Clock>,
    pub tokens: Arc<dyn TokenGenerator>,
    pub repo: Arc<dyn ModerationRepository>,
    pub audit: Arc<dyn AuditLog>,
    pub comments: Arc<dyn CommentRepository>,
    pub users: Arc<dyn UserRepository>,
    /// Discord ids allowed to moderate (`ADMIN_DISCORD_IDS`).
    pub admin_discord_ids: Vec<DiscordId>,
}

fn bad_request(message: &str) -> AppError {
    AppError::BadRequest(message.into())
}

fn parse_reason(raw: &str) -> PortResult<ModerationReason> {
    ModerationReason::parse(raw).map_err(bad_request)
}

impl ModerationService {
    pub fn is_admin(&self, user: &User) -> bool {
        is_admin(&self.admin_discord_ids, user)
    }

    fn require_admin(&self, user: &User) -> PortResult<()> {
        self.is_admin(user).then_some(()).ok_or(AppError::Forbidden)
    }

    async fn log(
        &self,
        admin: &User,
        action: ModerationAction,
        target: &str,
        reason: Option<&str>,
    ) -> PortResult<()> {
        record_audit(
            self.audit.as_ref(),
            self.clock.as_ref(),
            self.tokens.as_ref(),
            &admin.id,
            action,
            target.to_owned(),
            reason.map(str::to_owned),
        )
        .await
    }

    /// Refuses banned users (`Banned`). Called by every write route.
    pub async fn ensure_can_write(&self, user: &UserId) -> PortResult<()> {
        if self.repo.is_banned(user).await? {
            Err(AppError::Banned)
        } else {
            Ok(())
        }
    }

    /// Reports someone else's visible comment. Idempotent per (user, comment).
    /// Unknown, hidden or other-mod comments are `NotFound`; your own is a bad request.
    pub async fn report(
        &self,
        reporter: &User,
        mod_id: &str,
        comment_id: &str,
        reason: &str,
        details: Option<&str>,
    ) -> PortResult<()> {
        let mod_id = ModId::parse(mod_id).ok_or_else(|| bad_request("The mod id is invalid."))?;
        let reason = ReportReason::parse(reason)
            .ok_or_else(|| bad_request("The reason must be spam, abuse, off_topic or other."))?;
        let details = match details {
            Some(text) => parse_report_details(text).map_err(bad_request)?,
            None => None,
        };
        let id = CommentId::parse(comment_id).ok_or(AppError::NotFound)?;
        let found = self
            .comments
            .find(&mod_id, &id)
            .await?
            .ok_or(AppError::NotFound)?;
        if found.comment.user_id == reporter.id {
            return Err(bad_request("You cannot report your own comment."));
        }
        self.repo
            .insert_report(&Report {
                id: self.tokens.generate(16),
                comment_id: id,
                reporter_id: reporter.id.clone(),
                reason,
                details,
                created_at: self.clock.now(),
            })
            .await
    }

    /// Hides a visible comment; its open reports become `hidden`.
    pub async fn hide_comment(
        &self,
        admin: &User,
        comment_id: &str,
        reason: &str,
    ) -> PortResult<()> {
        self.require_admin(admin)?;
        let reason = parse_reason(reason)?;
        let id = CommentId::parse(comment_id).ok_or(AppError::NotFound)?;
        let hidden = self
            .repo
            .hide_comment(&id, &admin.id, reason.as_str(), self.clock.now())
            .await?;
        if !hidden {
            return Err(AppError::NotFound);
        }
        self.log(
            admin,
            ModerationAction::HideComment,
            id.as_str(),
            Some(reason.as_str()),
        )
        .await
    }

    pub async fn restore_comment(&self, admin: &User, comment_id: &str) -> PortResult<()> {
        self.require_admin(admin)?;
        let id = CommentId::parse(comment_id).ok_or(AppError::NotFound)?;
        if !self.repo.restore_comment(&id).await? {
            return Err(AppError::NotFound);
        }
        self.log(admin, ModerationAction::RestoreComment, id.as_str(), None)
            .await
    }

    /// Resolves an open report as dismissed; the audit target is its comment id.
    pub async fn dismiss_report(&self, admin: &User, report_id: &str) -> PortResult<()> {
        self.require_admin(admin)?;
        let comment = self
            .repo
            .dismiss_report(report_id, &admin.id, self.clock.now())
            .await?
            .ok_or(AppError::NotFound)?;
        self.log(
            admin,
            ModerationAction::DismissReport,
            comment.as_str(),
            None,
        )
        .await
    }

    /// Bans a user by internal id.
    pub async fn ban_user(&self, admin: &User, user_id: &str, reason: &str) -> PortResult<()> {
        self.require_admin(admin)?;
        let reason = parse_reason(reason)?;
        let target = self
            .users
            .get(&UserId(user_id.to_owned()))
            .await?
            .ok_or(AppError::NotFound)?;
        self.ban(admin, &target, &reason).await
    }

    /// Bans a user by Discord id. The user must have signed in at least once.
    pub async fn ban_by_discord_id(
        &self,
        admin: &User,
        discord_id: &str,
        reason: &str,
    ) -> PortResult<()> {
        self.require_admin(admin)?;
        let reason = parse_reason(reason)?;
        let discord_id = DiscordId::parse(discord_id)
            .ok_or_else(|| bad_request("The Discord id is invalid."))?;
        let target = self
            .repo
            .find_user_by_discord_id(&discord_id)
            .await?
            .ok_or_else(|| bad_request("No user with this Discord id has signed in yet."))?;
        self.ban(admin, &target, &reason).await
    }

    async fn ban(&self, admin: &User, target: &User, reason: &ModerationReason) -> PortResult<()> {
        if target.id == admin.id {
            return Err(bad_request("You cannot ban yourself."));
        }
        if self.is_admin(target) {
            return Err(bad_request("You cannot ban an administrator."));
        }
        self.repo
            .ban(&Ban {
                user_id: target.id.clone(),
                banned_by: Some(admin.id.clone()),
                reason: reason.as_str().to_owned(),
                banned_at: self.clock.now(),
            })
            .await?;
        self.log(
            admin,
            ModerationAction::BanUser,
            target.id.as_str(),
            Some(reason.as_str()),
        )
        .await
    }

    pub async fn unban(&self, admin: &User, user_id: &str) -> PortResult<()> {
        self.require_admin(admin)?;
        let id = UserId(user_id.to_owned());
        if !self.repo.unban(&id).await? {
            return Err(AppError::NotFound);
        }
        self.log(admin, ModerationAction::UnbanUser, id.as_str(), None)
            .await
    }

    /// Open reports grouped per comment, the comment with the oldest report first.
    pub async fn open_reports(&self, admin: &User) -> PortResult<Vec<ReportedComment>> {
        self.require_admin(admin)?;
        let mut groups: Vec<ReportedComment> = Vec::new();
        for (report, comment) in self.repo.open_reports().await? {
            match groups
                .iter_mut()
                .find(|group| group.comment.comment.id == report.comment_id)
            {
                Some(group) => group.reports.push(report),
                None => groups.push(ReportedComment {
                    comment,
                    reports: vec![report],
                }),
            }
        }
        Ok(groups)
    }

    pub async fn hidden_comments(&self, admin: &User) -> PortResult<Vec<HiddenComment>> {
        self.require_admin(admin)?;
        self.repo.hidden_comments().await
    }

    pub async fn bans(&self, admin: &User) -> PortResult<Vec<BannedUser>> {
        self.require_admin(admin)?;
        self.repo.bans().await
    }

    /// The newest `limit` audit entries first.
    pub async fn recent_audit(&self, admin: &User, limit: u32) -> PortResult<Vec<AuditEntry>> {
        self.require_admin(admin)?;
        self.repo.recent_audit(limit).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Duration;
    use community_domain::comments::{Comment, CommentBody, PageCursor};

    use super::*;

    struct TestClock(Mutex<DateTime<Utc>>);

    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            let mut now = self.0.lock().unwrap();
            *now += Duration::seconds(1);
            *now
        }
    }

    struct CountingTokens(Mutex<u32>);

    impl TokenGenerator for CountingTokens {
        fn generate(&self, _byte_len: usize) -> String {
            let mut n = self.0.lock().unwrap();
            *n += 1;
            format!("id{n:03}")
        }
    }

    fn user(id: &str, discord: &str) -> User {
        User {
            id: UserId(id.into()),
            discord_id: DiscordId::parse(discord).unwrap(),
            username: id.into(),
            global_name: None,
            avatar_hash: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    /// One in-memory store behind every port the service needs.
    #[derive(Default)]
    struct Memory {
        users: Mutex<Vec<User>>,
        comments: Mutex<Vec<(CommentWithAuthor, bool)>>,
        reports: Mutex<Vec<(Report, Option<&'static str>)>>,
        bans: Mutex<Vec<Ban>>,
        audit: Mutex<Vec<AuditEntry>>,
    }

    #[async_trait::async_trait]
    impl CommentRepository for Memory {
        async fn insert(&self, _comment: &Comment) -> PortResult<()> {
            unreachable!()
        }
        async fn page(
            &self,
            _mod_id: &ModId,
            _after: Option<&PageCursor>,
            _limit: u32,
        ) -> PortResult<Vec<CommentWithAuthor>> {
            unreachable!()
        }
        async fn find(
            &self,
            mod_id: &ModId,
            id: &CommentId,
        ) -> PortResult<Option<CommentWithAuthor>> {
            Ok(self
                .comments
                .lock()
                .unwrap()
                .iter()
                .find(|(c, hidden)| !hidden && &c.comment.mod_id == mod_id && &c.comment.id == id)
                .map(|(c, _)| c.clone()))
        }
        async fn update_body(
            &self,
            _mod_id: &ModId,
            _id: &CommentId,
            _author: &UserId,
            _body: &CommentBody,
            _edited_at: DateTime<Utc>,
        ) -> PortResult<bool> {
            unreachable!()
        }
        async fn delete(
            &self,
            _mod_id: &ModId,
            _id: &CommentId,
            _author: &UserId,
        ) -> PortResult<bool> {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl UserRepository for Memory {
        async fn upsert(&self, _user: &crate::ports::UserUpsert) -> PortResult<User> {
            unreachable!()
        }
        async fn get(&self, id: &UserId) -> PortResult<Option<User>> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .iter()
                .find(|u| &u.id == id)
                .cloned())
        }
    }

    #[async_trait::async_trait]
    impl AuditLog for Memory {
        async fn record(&self, entry: &AuditEntry) -> PortResult<()> {
            self.audit.lock().unwrap().push(entry.clone());
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ModerationRepository for Memory {
        async fn insert_report(&self, report: &Report) -> PortResult<()> {
            let mut reports = self.reports.lock().unwrap();
            let duplicate = reports.iter().any(|(r, _)| {
                r.comment_id == report.comment_id && r.reporter_id == report.reporter_id
            });
            if !duplicate {
                reports.push((report.clone(), None));
            }
            Ok(())
        }
        async fn open_reports(&self) -> PortResult<Vec<(Report, CommentWithAuthor)>> {
            let comments = self.comments.lock().unwrap();
            Ok(self
                .reports
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, resolution)| resolution.is_none())
                .filter_map(|(r, _)| {
                    comments
                        .iter()
                        .find(|(c, _)| c.comment.id == r.comment_id)
                        .map(|(c, _)| (r.clone(), c.clone()))
                })
                .collect())
        }
        async fn dismiss_report(
            &self,
            id: &str,
            _admin: &UserId,
            _now: DateTime<Utc>,
        ) -> PortResult<Option<CommentId>> {
            let mut reports = self.reports.lock().unwrap();
            Ok(reports
                .iter_mut()
                .find(|(r, resolution)| r.id == id && resolution.is_none())
                .map(|(r, resolution)| {
                    *resolution = Some("dismissed");
                    r.comment_id.clone()
                }))
        }
        async fn hide_comment(
            &self,
            id: &CommentId,
            _admin: &UserId,
            _reason: &str,
            _now: DateTime<Utc>,
        ) -> PortResult<bool> {
            let mut comments = self.comments.lock().unwrap();
            let Some((_, hidden)) = comments
                .iter_mut()
                .find(|(c, hidden)| !hidden && &c.comment.id == id)
            else {
                return Ok(false);
            };
            *hidden = true;
            for (report, resolution) in self.reports.lock().unwrap().iter_mut() {
                if &report.comment_id == id && resolution.is_none() {
                    *resolution = Some("hidden");
                }
            }
            Ok(true)
        }
        async fn restore_comment(&self, id: &CommentId) -> PortResult<bool> {
            let mut comments = self.comments.lock().unwrap();
            let found = comments
                .iter_mut()
                .find(|(c, hidden)| *hidden && &c.comment.id == id);
            Ok(found.map(|(_, hidden)| *hidden = false).is_some())
        }
        async fn hidden_comments(&self) -> PortResult<Vec<HiddenComment>> {
            Ok(self
                .comments
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, hidden)| *hidden)
                .map(|(c, _)| HiddenComment {
                    comment: c.clone(),
                    hidden_at: DateTime::from_timestamp(5, 0).unwrap(),
                    hidden_by: None,
                    reason: None,
                })
                .collect())
        }
        async fn ban(&self, ban: &Ban) -> PortResult<()> {
            let mut bans = self.bans.lock().unwrap();
            bans.retain(|b| b.user_id != ban.user_id);
            bans.push(ban.clone());
            Ok(())
        }
        async fn unban(&self, user: &UserId) -> PortResult<bool> {
            let mut bans = self.bans.lock().unwrap();
            let before = bans.len();
            bans.retain(|b| &b.user_id != user);
            Ok(bans.len() < before)
        }
        async fn is_banned(&self, user: &UserId) -> PortResult<bool> {
            Ok(self.bans.lock().unwrap().iter().any(|b| &b.user_id == user))
        }
        async fn bans(&self) -> PortResult<Vec<BannedUser>> {
            let users = self.users.lock().unwrap();
            Ok(self
                .bans
                .lock()
                .unwrap()
                .iter()
                .filter_map(|ban| {
                    users
                        .iter()
                        .find(|u| u.id == ban.user_id)
                        .map(|user| BannedUser {
                            user: user.clone(),
                            ban: ban.clone(),
                        })
                })
                .collect())
        }
        async fn find_user_by_discord_id(&self, id: &DiscordId) -> PortResult<Option<User>> {
            Ok(self
                .users
                .lock()
                .unwrap()
                .iter()
                .find(|u| &u.discord_id == id)
                .cloned())
        }
        async fn recent_audit(&self, limit: u32) -> PortResult<Vec<AuditEntry>> {
            let mut rows = self.audit.lock().unwrap().clone();
            rows.reverse();
            rows.truncate(limit as usize);
            Ok(rows)
        }
    }

    struct Fixture {
        service: ModerationService,
        memory: Arc<Memory>,
        admin: User,
        author: User,
        reader: User,
    }

    fn fixture() -> Fixture {
        let memory = Arc::new(Memory::default());
        let admin = user("admin", "999");
        let author = user("author", "1");
        let reader = user("reader", "2");
        memory
            .users
            .lock()
            .unwrap()
            .extend([admin.clone(), author.clone(), reader.clone()]);
        let service = ModerationService {
            clock: Arc::new(TestClock(Mutex::new(
                DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            ))),
            tokens: Arc::new(CountingTokens(Mutex::new(0))),
            repo: memory.clone(),
            audit: memory.clone(),
            comments: memory.clone(),
            users: memory.clone(),
            admin_discord_ids: vec![DiscordId::parse("999").unwrap()],
        };
        Fixture {
            service,
            memory,
            admin,
            author,
            reader,
        }
    }

    impl Fixture {
        fn add_comment(&self, id: &str, mod_id: &str, author: &User) {
            self.memory.comments.lock().unwrap().push((
                CommentWithAuthor {
                    comment: Comment {
                        id: CommentId(id.into()),
                        mod_id: ModId::parse(mod_id).unwrap(),
                        user_id: author.id.clone(),
                        body: CommentBody::parse("text").unwrap(),
                        created_at: DateTime::from_timestamp(10, 0).unwrap(),
                        edited_at: None,
                    },
                    author: author.clone(),
                },
                false,
            ));
        }

        fn audit_actions(&self) -> Vec<(ModerationAction, String, Option<String>)> {
            self.memory
                .audit
                .lock()
                .unwrap()
                .iter()
                .map(|e| (e.action, e.target.clone(), e.reason.clone()))
                .collect()
        }
    }

    #[tokio::test]
    async fn report_stores_one_report_and_is_idempotent() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        for _ in 0..2 {
            f.service
                .report(&f.reader, "mod-a", "c1", "spam", Some("  buy now  "))
                .await
                .unwrap();
        }
        let reports = f.memory.reports.lock().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].0.reason, ReportReason::Spam);
        assert_eq!(reports[0].0.details.as_deref(), Some("buy now"));
        assert_eq!(reports[0].0.reporter_id, f.reader.id);
    }

    #[tokio::test]
    async fn report_refuses_own_comments_bad_input_and_unknown_targets() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        let bad = |result: PortResult<()>| matches!(result, Err(AppError::BadRequest(_)));
        assert!(bad(f
            .service
            .report(&f.author, "mod-a", "c1", "spam", None)
            .await));
        assert!(bad(f
            .service
            .report(&f.reader, "mod-a", "c1", "nonsense", None)
            .await));
        assert!(bad(f
            .service
            .report(&f.reader, "mod-a", "c1", "other", Some(&"a".repeat(501)))
            .await));
        assert!(bad(f
            .service
            .report(&f.reader, "Bad Mod", "c1", "spam", None)
            .await));
        for (mod_id, comment) in [("mod-b", "c1"), ("mod-a", "nope"), ("mod-a", "bad id!")] {
            assert_eq!(
                f.service
                    .report(&f.reader, mod_id, comment, "spam", None)
                    .await,
                Err(AppError::NotFound)
            );
        }
        assert!(f.memory.reports.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn report_of_a_hidden_comment_is_not_found() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        f.service
            .hide_comment(&f.admin, "c1", "rude")
            .await
            .unwrap();
        assert_eq!(
            f.service
                .report(&f.reader, "mod-a", "c1", "spam", None)
                .await,
            Err(AppError::NotFound)
        );
    }

    #[tokio::test]
    async fn open_reports_group_per_comment_with_count_and_reasons() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        f.add_comment("c2", "mod-a", &f.author);
        let third = user("third", "3");
        f.memory.users.lock().unwrap().push(third.clone());
        f.service
            .report(&f.reader, "mod-a", "c1", "spam", None)
            .await
            .unwrap();
        f.service
            .report(&third, "mod-a", "c1", "spam", None)
            .await
            .unwrap();
        f.service
            .report(&f.admin, "mod-a", "c1", "abuse", None)
            .await
            .unwrap();
        f.service
            .report(&f.reader, "mod-a", "c2", "other", None)
            .await
            .unwrap();

        let groups = f.service.open_reports(&f.admin).await.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].comment.comment.id.as_str(), "c1");
        assert_eq!(groups[0].report_count(), 3);
        assert_eq!(
            groups[0].reasons(),
            [ReportReason::Spam, ReportReason::Abuse]
        );
        assert_eq!(groups[1].report_count(), 1);
    }

    #[tokio::test]
    async fn hiding_resolves_reports_and_writes_the_audit() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        f.service
            .report(&f.reader, "mod-a", "c1", "spam", None)
            .await
            .unwrap();
        f.service
            .hide_comment(&f.admin, "c1", "  advertising ")
            .await
            .unwrap();

        assert!(f.service.open_reports(&f.admin).await.unwrap().is_empty());
        assert_eq!(f.service.hidden_comments(&f.admin).await.unwrap().len(), 1);
        assert_eq!(
            f.audit_actions(),
            [(
                ModerationAction::HideComment,
                "c1".to_owned(),
                Some("advertising".to_owned())
            )]
        );
        let entry = f.memory.audit.lock().unwrap()[0].clone();
        assert_eq!(entry.admin_id, Some(f.admin.id.clone()));
        assert!(entry.created_at > DateTime::from_timestamp(1_700_000_000, 0).unwrap());
    }

    #[tokio::test]
    async fn hide_needs_a_reason_a_visible_comment_and_an_admin() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        assert!(matches!(
            f.service.hide_comment(&f.admin, "c1", "  ").await,
            Err(AppError::BadRequest(_))
        ));
        assert_eq!(
            f.service.hide_comment(&f.admin, "nope", "x").await,
            Err(AppError::NotFound)
        );
        assert_eq!(
            f.service.hide_comment(&f.admin, "bad id!", "x").await,
            Err(AppError::NotFound)
        );
        assert_eq!(
            f.service.hide_comment(&f.reader, "c1", "x").await,
            Err(AppError::Forbidden)
        );
        f.service.hide_comment(&f.admin, "c1", "x").await.unwrap();
        // Already hidden.
        assert_eq!(
            f.service.hide_comment(&f.admin, "c1", "x").await,
            Err(AppError::NotFound)
        );
        assert_eq!(f.audit_actions().len(), 1);
    }

    #[tokio::test]
    async fn restore_unhides_and_is_audited() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        assert_eq!(
            f.service.restore_comment(&f.admin, "c1").await,
            Err(AppError::NotFound)
        );
        assert_eq!(
            f.service.restore_comment(&f.admin, "bad id!").await,
            Err(AppError::NotFound)
        );
        assert_eq!(
            f.service.restore_comment(&f.reader, "c1").await,
            Err(AppError::Forbidden)
        );
        f.service.hide_comment(&f.admin, "c1", "x").await.unwrap();
        f.service.restore_comment(&f.admin, "c1").await.unwrap();
        assert!(f
            .service
            .hidden_comments(&f.admin)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(f.audit_actions()[1].0, ModerationAction::RestoreComment);
    }

    #[tokio::test]
    async fn dismissing_a_report_resolves_it_and_audits_the_comment() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        f.service
            .report(&f.reader, "mod-a", "c1", "spam", None)
            .await
            .unwrap();
        let report_id = f.memory.reports.lock().unwrap()[0].0.id.clone();
        assert_eq!(
            f.service.dismiss_report(&f.reader, &report_id).await,
            Err(AppError::Forbidden)
        );
        f.service
            .dismiss_report(&f.admin, &report_id)
            .await
            .unwrap();
        assert!(f.service.open_reports(&f.admin).await.unwrap().is_empty());
        assert_eq!(
            f.audit_actions(),
            [(ModerationAction::DismissReport, "c1".to_owned(), None)]
        );
        assert_eq!(
            f.service.dismiss_report(&f.admin, &report_id).await,
            Err(AppError::NotFound)
        );
    }

    #[tokio::test]
    async fn banned_users_cannot_write_until_unbanned() {
        let f = fixture();
        f.service.ensure_can_write(&f.author.id).await.unwrap();
        f.service
            .ban_user(&f.admin, f.author.id.as_str(), "  spam wave ")
            .await
            .unwrap();
        assert_eq!(
            f.service.ensure_can_write(&f.author.id).await,
            Err(AppError::Banned)
        );
        f.service.ensure_can_write(&f.reader.id).await.unwrap();

        let bans = f.service.bans(&f.admin).await.unwrap();
        assert_eq!(bans.len(), 1);
        assert_eq!(bans[0].ban.reason, "spam wave");
        assert_eq!(bans[0].ban.banned_by, Some(f.admin.id.clone()));

        f.service
            .unban(&f.admin, f.author.id.as_str())
            .await
            .unwrap();
        f.service.ensure_can_write(&f.author.id).await.unwrap();
        assert_eq!(
            f.service.unban(&f.admin, f.author.id.as_str()).await,
            Err(AppError::NotFound)
        );
        let actions: Vec<_> = f.audit_actions().into_iter().map(|a| a.0).collect();
        assert_eq!(
            actions,
            [ModerationAction::BanUser, ModerationAction::UnbanUser]
        );
    }

    #[tokio::test]
    async fn banning_by_discord_id_works_and_checks_the_target() {
        let f = fixture();
        f.service
            .ban_by_discord_id(&f.admin, " 2 ", "abuse")
            .await
            .unwrap();
        assert_eq!(
            f.service.ensure_can_write(&f.reader.id).await,
            Err(AppError::Banned)
        );
        // Unknown and malformed Discord ids.
        for id in ["404", "abc"] {
            assert!(matches!(
                f.service.ban_by_discord_id(&f.admin, id, "x").await,
                Err(AppError::BadRequest(_))
            ));
        }
        assert!(matches!(
            f.service.ban_by_discord_id(&f.admin, "1", " ").await,
            Err(AppError::BadRequest(_))
        ));
        assert_eq!(
            f.service.ban_by_discord_id(&f.reader, "1", "x").await,
            Err(AppError::Forbidden)
        );
    }

    #[tokio::test]
    async fn an_admin_cannot_ban_themselves_or_another_admin() {
        let f = fixture();
        let second_admin = user("admin2", "888");
        f.memory.users.lock().unwrap().push(second_admin.clone());
        let service = ModerationService {
            clock: f.service.clock.clone(),
            tokens: f.service.tokens.clone(),
            repo: f.memory.clone(),
            audit: f.memory.clone(),
            comments: f.memory.clone(),
            users: f.memory.clone(),
            admin_discord_ids: vec![
                DiscordId::parse("999").unwrap(),
                DiscordId::parse("888").unwrap(),
            ],
        };
        for (by_id, discord) in [(f.admin.id.as_str(), "999"), ("admin2", "888")] {
            assert!(matches!(
                service.ban_user(&f.admin, by_id, "x").await,
                Err(AppError::BadRequest(_))
            ));
            assert!(matches!(
                service.ban_by_discord_id(&f.admin, discord, "x").await,
                Err(AppError::BadRequest(_))
            ));
        }
        assert!(f.memory.bans.lock().unwrap().is_empty());
        assert_eq!(
            service.ban_user(&f.admin, "ghost", "x").await,
            Err(AppError::NotFound)
        );
    }

    #[tokio::test]
    async fn recent_audit_lists_newest_first_and_needs_an_admin() {
        let f = fixture();
        f.add_comment("c1", "mod-a", &f.author);
        f.service.hide_comment(&f.admin, "c1", "x").await.unwrap();
        f.service.restore_comment(&f.admin, "c1").await.unwrap();
        let rows = f.service.recent_audit(&f.admin, 1).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, ModerationAction::RestoreComment);
        assert_eq!(
            f.service.recent_audit(&f.reader, 1).await,
            Err(AppError::Forbidden)
        );
        assert_eq!(
            f.service.open_reports(&f.reader).await,
            Err(AppError::Forbidden)
        );
        assert_eq!(f.service.bans(&f.reader).await, Err(AppError::Forbidden));
        assert_eq!(
            f.service.hidden_comments(&f.reader).await,
            Err(AppError::Forbidden)
        );
    }

    #[test]
    fn is_admin_matches_the_discord_id() {
        let admins = [DiscordId::parse("999").unwrap()];
        assert!(is_admin(&admins, &user("a", "999")));
        assert!(!is_admin(&admins, &user("b", "1")));
    }
}
