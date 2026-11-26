//! Moderation: report reasons, admin reasons, audit entries, reports and bans. No IO.

use chrono::{DateTime, Utc};

use crate::comments::CommentId;
use crate::UserId;

/// Longest admin reason and longest report details, in `char`s after trimming.
pub const MAX_REASON_CHARS: usize = 500;

/// Why a user reports a comment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportReason {
    Spam,
    Abuse,
    OffTopic,
    Other,
}

impl ReportReason {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "spam" => Some(Self::Spam),
            "abuse" => Some(Self::Abuse),
            "off_topic" => Some(Self::OffTopic),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spam => "spam",
            Self::Abuse => "abuse",
            Self::OffTopic => "off_topic",
            Self::Other => "other",
        }
    }
}

/// Free text of a report: trimmed, at most 500 chars, empty means none.
pub fn parse_report_details(raw: &str) -> Result<Option<String>, &'static str> {
    let text = raw.trim();
    if text.chars().count() > MAX_REASON_CHARS {
        Err("The details are longer than 500 characters.")
    } else if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text.to_owned()))
    }
}

/// A moderator's reason: trimmed, 1 to 500 chars.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModerationReason(String);

impl ModerationReason {
    pub fn parse(raw: &str) -> Result<Self, &'static str> {
        let text = raw.trim();
        if text.is_empty() || text.chars().count() > MAX_REASON_CHARS {
            return Err("The reason is required and at most 500 characters.");
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What an admin did (one value per audit row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModerationAction {
    HideComment,
    RestoreComment,
    BanUser,
    UnbanUser,
    DismissReport,
    ApproveVersion,
    RejectVersion,
}

impl ModerationAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HideComment => "hide_comment",
            Self::RestoreComment => "restore_comment",
            Self::BanUser => "ban_user",
            Self::UnbanUser => "unban_user",
            Self::DismissReport => "dismiss_report",
            Self::ApproveVersion => "approve_version",
            Self::RejectVersion => "reject_version",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [
            Self::HideComment,
            Self::RestoreComment,
            Self::BanUser,
            Self::UnbanUser,
            Self::DismissReport,
            Self::ApproveVersion,
            Self::RejectVersion,
        ]
        .into_iter()
        .find(|action| action.as_str() == value)
    }
}

/// One audit row: who did what to which target, when and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: String,
    /// `None` once the admin's user row is gone.
    pub admin_id: Option<UserId>,
    /// The admin's display name; filled only when reading the log.
    pub admin_name: Option<String>,
    pub action: ModerationAction,
    /// A comment id, a user id or `mod@version`.
    pub target: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A user's report of a comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub id: String,
    pub comment_id: CommentId,
    pub reporter_id: UserId,
    pub reason: ReportReason,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A ban of a user from community writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ban {
    pub user_id: UserId,
    pub banned_by: Option<UserId>,
    pub reason: String,
    pub banned_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_reasons_round_trip() {
        for reason in [
            ReportReason::Spam,
            ReportReason::Abuse,
            ReportReason::OffTopic,
            ReportReason::Other,
        ] {
            assert_eq!(ReportReason::parse(reason.as_str()), Some(reason));
        }
        assert_eq!(ReportReason::parse("Spam"), None);
        assert_eq!(ReportReason::parse(""), None);
    }

    #[test]
    fn report_details_are_trimmed_and_bounded() {
        assert_eq!(parse_report_details("  "), Ok(None));
        assert_eq!(parse_report_details(" rude "), Ok(Some("rude".into())));
        assert!(parse_report_details(&"a".repeat(500)).is_ok());
        assert!(parse_report_details(&"a".repeat(501)).is_err());
    }

    #[test]
    fn moderation_reason_is_required_trimmed_and_bounded() {
        assert_eq!(
            ModerationReason::parse("  spam  ").unwrap().as_str(),
            "spam"
        );
        assert!(ModerationReason::parse("   ").is_err());
        assert!(ModerationReason::parse("").is_err());
        assert!(ModerationReason::parse(&"a".repeat(500)).is_ok());
        assert!(ModerationReason::parse(&"a".repeat(501)).is_err());
        // Counted in characters, not bytes.
        assert!(ModerationReason::parse(&"é".repeat(500)).is_ok());
    }

    #[test]
    fn actions_round_trip() {
        for action in [
            ModerationAction::HideComment,
            ModerationAction::RestoreComment,
            ModerationAction::BanUser,
            ModerationAction::UnbanUser,
            ModerationAction::DismissReport,
            ModerationAction::ApproveVersion,
            ModerationAction::RejectVersion,
        ] {
            assert_eq!(ModerationAction::parse(action.as_str()), Some(action));
        }
        assert_eq!(ModerationAction::parse("nope"), None);
    }
}
