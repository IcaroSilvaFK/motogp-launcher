//! Mod comments (issue #118): what the launcher shows and the rules a draft
//! must follow before it is sent to the community server.
use std::fmt;

/// Longest comment, counted in characters after trimming (same as the server).
pub const MAX_COMMENT_CHARS: usize = 2000;

/// One comment as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModComment {
    pub id: String,
    pub author_name: String,
    /// The avatar as a `data:` URL (the WebView CSP only allows `data:` images).
    pub author_avatar_data_url: Option<String>,
    /// Plain text; the UI renders it with the safe Markdown renderer.
    pub body: String,
    /// Creation time, milliseconds since the Unix epoch.
    pub created_at_ms: i64,
    pub edited: bool,
    /// Written by the signed-in member: only these can be edited or deleted.
    pub own: bool,
}

/// One page of comments, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentsPage {
    pub comments: Vec<ModComment>,
    /// Opaque; pass it back unchanged to read the next page.
    pub next_cursor: Option<String>,
}

/// Why a draft cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentDraftError {
    Empty,
    TooLong,
    ControlChar,
}

impl fmt::Display for CommentDraftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommentDraftError::Empty => write!(f, "the comment is empty"),
            CommentDraftError::TooLong => {
                write!(
                    f,
                    "the comment is longer than {MAX_COMMENT_CHARS} characters"
                )
            }
            CommentDraftError::ControlChar => {
                write!(f, "the comment contains control characters")
            }
        }
    }
}

impl std::error::Error for CommentDraftError {}

/// A comment body that follows the server's rules: trimmed, `\r\n` turned into
/// `\n`, 1 to [`MAX_COMMENT_CHARS`] characters, no control characters other
/// than `\n` and `\t`. Parsing fails fast, so nothing invalid reaches the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDraft(String);

impl CommentDraft {
    pub fn parse(raw: &str) -> Result<Self, CommentDraftError> {
        let body = raw.replace("\r\n", "\n");
        let body = body.trim();
        if body.is_empty() {
            return Err(CommentDraftError::Empty);
        }
        if body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(CommentDraftError::ControlChar);
        }
        if body.chars().count() > MAX_COMMENT_CHARS {
            return Err(CommentDraftError::TooLong);
        }
        Ok(Self(body.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Longest optional detail text of a report, counted in characters.
pub const MAX_REPORT_DETAILS_CHARS: usize = 500;

/// Why a member reports a comment (the server's wire values).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportReason {
    Spam,
    Abuse,
    OffTopic,
    Other,
}

impl ReportReason {
    /// Parses the wire value (`spam`, `abuse`, `off_topic`, `other`).
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

/// Report details longer than [`MAX_REPORT_DETAILS_CHARS`] or with control
/// characters other than `\n` and `\t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidReportDetails;

/// Trims the optional report details; blank becomes `None`.
pub fn parse_report_details(raw: Option<&str>) -> Result<Option<String>, InvalidReportDetails> {
    let Some(raw) = raw.map(str::trim).filter(|d| !d.is_empty()) else {
        return Ok(None);
    };
    if raw.chars().count() > MAX_REPORT_DETAILS_CHARS
        || raw
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(InvalidReportDetails);
    }
    Ok(Some(raw.to_string()))
}

/// Whether `value` is a catalog slug the server accepts as a mod id: 1 to 100
/// characters of `a-z`, `0-9` and `-`.
pub fn is_valid_catalog_slug(value: &str) -> bool {
    (1..=100).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_reasons_round_trip_their_wire_values() {
        for (wire, reason) in [
            ("spam", ReportReason::Spam),
            ("abuse", ReportReason::Abuse),
            ("off_topic", ReportReason::OffTopic),
            ("other", ReportReason::Other),
        ] {
            assert_eq!(ReportReason::parse(wire), Some(reason));
            assert_eq!(reason.as_str(), wire);
        }
        assert_eq!(ReportReason::parse("Spam"), None);
        assert_eq!(ReportReason::parse(""), None);
    }

    #[test]
    fn report_details_are_trimmed_and_bounded() {
        assert_eq!(parse_report_details(None), Ok(None));
        assert_eq!(parse_report_details(Some("  \n ")), Ok(None));
        assert_eq!(
            parse_report_details(Some("  rude  ")),
            Ok(Some("rude".to_string()))
        );
        let max = "a".repeat(MAX_REPORT_DETAILS_CHARS);
        assert!(parse_report_details(Some(&max)).is_ok());
        let over = "a".repeat(MAX_REPORT_DETAILS_CHARS + 1);
        assert_eq!(parse_report_details(Some(&over)), Err(InvalidReportDetails));
        assert_eq!(
            parse_report_details(Some("bad\u{7}")),
            Err(InvalidReportDetails)
        );
    }

    #[test]
    fn a_draft_is_trimmed_and_line_endings_are_normalized() {
        let draft = CommentDraft::parse("  hello\r\nworld\t \n").unwrap();
        assert_eq!(draft.as_str(), "hello\nworld");
    }

    #[test]
    fn an_empty_or_blank_draft_is_rejected() {
        assert_eq!(CommentDraft::parse(""), Err(CommentDraftError::Empty));
        assert_eq!(CommentDraft::parse(" \n\t "), Err(CommentDraftError::Empty));
    }

    #[test]
    fn the_length_cap_counts_characters_not_bytes() {
        let ok = "é".repeat(MAX_COMMENT_CHARS);
        assert!(CommentDraft::parse(&ok).is_ok());
        let too_long = "a".repeat(MAX_COMMENT_CHARS + 1);
        assert_eq!(
            CommentDraft::parse(&too_long),
            Err(CommentDraftError::TooLong)
        );
    }

    #[test]
    fn control_characters_other_than_newline_and_tab_are_rejected() {
        assert_eq!(
            CommentDraft::parse("a\u{0007}b"),
            Err(CommentDraftError::ControlChar)
        );
        assert!(CommentDraft::parse("a\tb\nc").is_ok());
    }

    #[test]
    fn html_is_kept_verbatim() {
        let draft = CommentDraft::parse("<img src=x onerror=alert(1)>").unwrap();
        assert_eq!(draft.as_str(), "<img src=x onerror=alert(1)>");
    }

    #[test]
    fn errors_have_readable_messages() {
        for error in [
            CommentDraftError::Empty,
            CommentDraftError::TooLong,
            CommentDraftError::ControlChar,
        ] {
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn catalog_slugs_follow_the_server_rule() {
        assert!(is_valid_catalog_slug("red-bull-livery-2"));
        assert!(is_valid_catalog_slug(&"a".repeat(100)));
        assert!(!is_valid_catalog_slug(""));
        assert!(!is_valid_catalog_slug(&"a".repeat(101)));
        assert!(!is_valid_catalog_slug("Upper"));
        assert!(!is_valid_catalog_slug("a/b"));
        assert!(!is_valid_catalog_slug("a b"));
    }
}
