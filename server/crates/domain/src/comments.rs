//! Mod comments: identifiers, the body rules and the pagination cursor. No IO.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};

use crate::UserId;

/// Longest comment body, counted in `char`s after trimming.
pub const MAX_COMMENT_CHARS: usize = 2000;
/// Longest mod slug.
pub const MAX_MOD_ID_CHARS: usize = 100;
/// Page size when the client does not ask for one.
pub const DEFAULT_PAGE_SIZE: u32 = 20;
/// Largest page a client may ask for.
pub const MAX_PAGE_SIZE: u32 = 50;

const MAX_COMMENT_ID_CHARS: usize = 64;

/// The catalog slug of a mod: 1 to 100 characters of `a-z 0-9 -`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModId(String);

impl ModId {
    pub fn parse(value: &str) -> Option<Self> {
        let valid = !value.is_empty()
            && value.len() <= MAX_MOD_ID_CHARS
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque comment id (base64url, as produced by the token generator).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentId(pub String);

impl CommentId {
    /// Returns `None` unless the value looks like a generated id.
    pub fn parse(value: &str) -> Option<Self> {
        let valid = !value.is_empty()
            && value.len() <= MAX_COMMENT_ID_CHARS
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a comment body was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentBodyError {
    Empty,
    TooLong,
    ControlChar,
}

impl CommentBodyError {
    /// English message that is safe to show to clients.
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "The comment is empty.",
            Self::TooLong => "The comment is longer than 2000 characters.",
            Self::ControlChar => "The comment contains control characters.",
        }
    }
}

/// A validated comment body: CRLF normalized to LF, trimmed, 1..=2000 chars,
/// no control characters other than newline and tab. It is plain text: HTML is
/// kept verbatim and never interpreted by the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentBody(String);

impl CommentBody {
    pub fn parse(raw: &str) -> Result<Self, CommentBodyError> {
        let normalized = raw.replace("\r\n", "\n");
        let body = normalized.trim();
        if body.is_empty() {
            return Err(CommentBodyError::Empty);
        }
        if body.chars().count() > MAX_COMMENT_CHARS {
            return Err(CommentBodyError::TooLong);
        }
        if body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(CommentBodyError::ControlChar);
        }
        Ok(Self(body.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stored comment (the author's profile is joined in by the read model).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub id: CommentId,
    pub mod_id: ModId,
    pub user_id: UserId,
    pub body: CommentBody,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

/// Keyset position `(created_at_ms, id)` after which the next page starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageCursor {
    pub created_at_ms: i64,
    pub id: String,
}

impl PageCursor {
    /// Opaque text form handed to clients.
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(format!("{}.{}", self.created_at_ms, self.id))
    }

    /// `None` for anything that was not produced by [`PageCursor::encode`].
    pub fn decode(value: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
        let text = String::from_utf8(bytes).ok()?;
        let (ms, id) = text.split_once('.')?;
        let id = CommentId::parse(id)?;
        Some(Self {
            created_at_ms: ms.parse().ok()?,
            id: id.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_id_accepts_catalog_slugs_only() {
        assert!(ModId::parse("red-bull-livery-2026").is_some());
        assert!(ModId::parse("a").is_some());
        assert!(ModId::parse(&"a".repeat(100)).is_some());
        assert!(ModId::parse("").is_none());
        assert!(ModId::parse(&"a".repeat(101)).is_none());
        assert!(ModId::parse("Upper").is_none());
        assert!(ModId::parse("with space").is_none());
        assert!(ModId::parse("a/b").is_none());
        assert!(ModId::parse("under_score").is_none());
        assert!(ModId::parse("cafe\u{301}").is_none());
    }

    #[test]
    fn comment_id_accepts_generated_ids_only() {
        assert!(CommentId::parse("Ab-_09").is_some());
        assert!(CommentId::parse("").is_none());
        assert!(CommentId::parse(&"a".repeat(65)).is_none());
        assert!(CommentId::parse("a.b").is_none());
    }

    #[test]
    fn body_is_trimmed_and_crlf_normalized() {
        let body = CommentBody::parse("  one\r\ntwo\t \n").unwrap();
        assert_eq!(body.as_str(), "one\ntwo");
    }

    #[test]
    fn body_rejects_empty_and_whitespace_only() {
        assert_eq!(CommentBody::parse(""), Err(CommentBodyError::Empty));
        assert_eq!(CommentBody::parse(" \r\n\t "), Err(CommentBodyError::Empty));
    }

    #[test]
    fn body_length_counts_chars_not_bytes() {
        let ok = "é".repeat(MAX_COMMENT_CHARS);
        assert!(CommentBody::parse(&ok).is_ok());
        let long = "é".repeat(MAX_COMMENT_CHARS + 1);
        assert_eq!(CommentBody::parse(&long), Err(CommentBodyError::TooLong));
    }

    #[test]
    fn body_length_is_measured_after_trimming() {
        let padded = format!("  {}  ", "a".repeat(MAX_COMMENT_CHARS));
        assert!(CommentBody::parse(&padded).is_ok());
    }

    #[test]
    fn body_rejects_control_characters_but_keeps_newline_and_tab() {
        assert_eq!(
            CommentBody::parse("bad\u{0}byte"),
            Err(CommentBodyError::ControlChar)
        );
        assert_eq!(
            CommentBody::parse("lone\rcr"),
            Err(CommentBodyError::ControlChar)
        );
        assert_eq!(
            CommentBody::parse("esc\u{1b}[0m"),
            Err(CommentBodyError::ControlChar)
        );
        assert!(CommentBody::parse("a\tb\nc").is_ok());
    }

    #[test]
    fn body_keeps_raw_html_verbatim() {
        let html = "<img src=x onerror=alert(1)>";
        assert_eq!(CommentBody::parse(html).unwrap().as_str(), html);
    }

    #[test]
    fn every_body_error_has_a_message() {
        for error in [
            CommentBodyError::Empty,
            CommentBodyError::TooLong,
            CommentBodyError::ControlChar,
        ] {
            assert!(!error.message().is_empty());
        }
    }

    #[test]
    fn cursor_round_trips_and_is_opaque() {
        let cursor = PageCursor {
            created_at_ms: 1_700_000_000_123,
            id: "abc-_9".into(),
        };
        let text = cursor.encode();
        assert!(!text.contains("1700000000123"));
        assert_eq!(PageCursor::decode(&text), Some(cursor));
    }

    #[test]
    fn cursor_rejects_garbage() {
        assert_eq!(PageCursor::decode("%%%"), None);
        assert_eq!(PageCursor::decode(""), None);
        assert_eq!(PageCursor::decode(&URL_SAFE_NO_PAD.encode("nodot")), None);
        assert_eq!(PageCursor::decode(&URL_SAFE_NO_PAD.encode("x.id")), None);
        assert_eq!(PageCursor::decode(&URL_SAFE_NO_PAD.encode("12.")), None);
        assert_eq!(PageCursor::decode(&URL_SAFE_NO_PAD.encode("12.a b")), None);
        assert_eq!(
            PageCursor::decode(&URL_SAFE_NO_PAD.encode([0xff, 0xfe])),
            None
        );
    }
}
