//! Online mod catalog entries (issue #72). Pure validation, no I/O.
//!
//! The catalog is a static JSON file maintained in the project repository.
//! Adapters parse it into [`CatalogEntryDraft`]s (unvalidated fields) and the
//! rules here turn each draft into a trusted [`CatalogEntry`] or reject it.

use std::fmt;

/// Highest `schemaVersion` of the catalog file this build understands.
pub const SUPPORTED_CATALOG_SCHEMA: u32 = 1;

/// Longest accepted entry id.
const MAX_ID_LEN: usize = 64;

/// Unvalidated download fields as read from the catalog file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogDownloadDraft {
    pub url: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

/// Unvalidated catalog entry as read from the catalog file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogEntryDraft {
    pub id: String,
    pub title: String,
    pub author: String,
    pub game: String,
    pub category: String,
    pub description: String,
    pub version: String,
    /// `YYYY-MM-DD`.
    pub updated_at: String,
    pub page_url: String,
    pub thumbnail_url: Option<String>,
    pub download: Option<CatalogDownloadDraft>,
}

/// A validated direct download: https URL, positive size and lowercase
/// SHA-256 of the file. Present only when the mod author allows redistribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDownload {
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// A validated catalog entry. Every string is trimmed, `id` is a slug, all
/// URLs are https and `updated_at` is a `YYYY-MM-DD` date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    pub game: String,
    pub category: String,
    pub description: String,
    pub version: String,
    pub updated_at: String,
    pub page_url: String,
    pub thumbnail_url: Option<String>,
    pub download: Option<CatalogDownload>,
}

/// Why a [`CatalogEntryDraft`] was rejected. Names the offending field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogEntryError {
    /// A required text field is empty or whitespace only.
    Blank { field: &'static str },
    /// The id is not a lowercase slug of at most 64 characters.
    InvalidId,
    /// A URL is not a well-formed `https://` URL (no host, spaces or credentials).
    InsecureUrl { field: &'static str },
    /// `updatedAt` is not a real `YYYY-MM-DD` date.
    InvalidDate,
    /// The sha256 is not 64 lowercase hex characters.
    InvalidSha256,
    /// The download size is zero.
    InvalidSize,
    /// A download needs url, size and sha256 together.
    IncompleteDownload,
}

impl fmt::Display for CatalogEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogEntryError::Blank { field } => write!(f, "{field} is blank"),
            CatalogEntryError::InvalidId => {
                write!(
                    f,
                    "id must be a lowercase slug of at most {MAX_ID_LEN} characters"
                )
            }
            CatalogEntryError::InsecureUrl { field } => {
                write!(f, "{field} must be a valid https URL")
            }
            CatalogEntryError::InvalidDate => write!(f, "updatedAt must be a YYYY-MM-DD date"),
            CatalogEntryError::InvalidSha256 => {
                write!(f, "sha256 must be 64 lowercase hex characters")
            }
            CatalogEntryError::InvalidSize => write!(f, "download size must be greater than zero"),
            CatalogEntryError::IncompleteDownload => {
                write!(f, "download needs url, size and sha256")
            }
        }
    }
}

impl std::error::Error for CatalogEntryError {}

impl CatalogEntry {
    /// Validates a draft. The first broken rule wins.
    pub fn from_draft(draft: CatalogEntryDraft) -> Result<CatalogEntry, CatalogEntryError> {
        let id = draft.id.trim().to_string();
        if !is_slug(&id) {
            return Err(CatalogEntryError::InvalidId);
        }
        let title = required("title", &draft.title)?;
        let author = required("author", &draft.author)?;
        let game = required("game", &draft.game)?;
        let category = required("category", &draft.category)?;
        let description = required("description", &draft.description)?;
        let version = required("version", &draft.version)?;
        let updated_at = draft.updated_at.trim().to_string();
        if !is_date(&updated_at) {
            return Err(CatalogEntryError::InvalidDate);
        }
        let page_url = https_url("pageUrl", &draft.page_url)?;
        let thumbnail_url = draft
            .thumbnail_url
            .as_deref()
            .map(|url| https_url("thumbnailUrl", url))
            .transpose()?;
        let download = draft.download.map(validate_download).transpose()?;
        Ok(CatalogEntry {
            id,
            title,
            author,
            game,
            category,
            description,
            version,
            updated_at,
            page_url,
            thumbnail_url,
            download,
        })
    }
}

fn validate_download(draft: CatalogDownloadDraft) -> Result<CatalogDownload, CatalogEntryError> {
    let url = https_url("download.url", &draft.url)?;
    let (Some(size), Some(sha256)) = (draft.size, draft.sha256) else {
        return Err(CatalogEntryError::IncompleteDownload);
    };
    if size == 0 {
        return Err(CatalogEntryError::InvalidSize);
    }
    let sha256 = sha256.trim().to_string();
    let hex = sha256.len() == 64
        && sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !hex {
        return Err(CatalogEntryError::InvalidSha256);
    }
    Ok(CatalogDownload { url, size, sha256 })
}

fn required(field: &'static str, value: &str) -> Result<String, CatalogEntryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CatalogEntryError::Blank { field });
    }
    Ok(trimmed.to_string())
}

/// `^[a-z0-9]+(-[a-z0-9]+)*$`, at most 64 characters.
fn is_slug(id: &str) -> bool {
    id.len() <= MAX_ID_LEN
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// A real calendar date written as `YYYY-MM-DD`.
fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = &value[range];
        if part.bytes().all(|b| b.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (Some(year), Some(month), Some(day)) = (digits(0..4), digits(5..7), digits(8..10)) else {
        return false;
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// `https://` scheme (case-insensitive), a non-empty host, no whitespace and
/// no `user@` credentials.
fn https_url(field: &'static str, value: &str) -> Result<String, CatalogEntryError> {
    well_formed_https_url(value).ok_or(CatalogEntryError::InsecureUrl { field })
}

/// The shared well-formed `https://` rule: the trimmed URL, or `None` when the
/// scheme is not https, the host is missing, or it has whitespace or
/// credentials. Also used by the ad creative rules.
pub(crate) fn well_formed_https_url(value: &str) -> Option<String> {
    let url = value.trim();
    let scheme_ok = url
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
    if !scheme_ok || url.chars().any(char::is_whitespace) {
        return None;
    }
    let rest = &url[8..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn draft() -> CatalogEntryDraft {
        CatalogEntryDraft {
            id: "rebalanced-ai".into(),
            title: "Rebalanced AI".into(),
            author: "Author".into(),
            game: "MotoGP 26".into(),
            category: "Gameplay".into(),
            description: "Tweaks the AI.".into(),
            version: "1.0".into(),
            updated_at: "2026-11-20".into(),
            page_url: "https://example.com/mod".into(),
            thumbnail_url: None,
            download: None,
        }
    }

    fn download(url: &str, size: Option<u64>, sha: Option<&str>) -> Option<CatalogDownloadDraft> {
        Some(CatalogDownloadDraft {
            url: url.into(),
            size,
            sha256: sha.map(String::from),
        })
    }

    #[test]
    fn complete_draft_is_valid_and_trimmed() {
        let mut d = draft();
        d.title = "  Rebalanced AI  ".into();
        d.thumbnail_url = Some(" https://example.com/t.png ".into());
        d.download = download("https://example.com/f.zip", Some(10), Some(SHA));
        let e = CatalogEntry::from_draft(d).unwrap();
        assert_eq!(e.title, "Rebalanced AI");
        assert_eq!(
            e.thumbnail_url.as_deref(),
            Some("https://example.com/t.png")
        );
        let dl = e.download.unwrap();
        assert_eq!((dl.size, dl.sha256.as_str()), (10, SHA));
    }

    #[test]
    fn entry_without_optional_fields_is_valid() {
        let e = CatalogEntry::from_draft(draft()).unwrap();
        assert!(e.thumbnail_url.is_none() && e.download.is_none());
    }

    #[test]
    fn blank_required_fields_name_the_field() {
        type Set = fn(&mut CatalogEntryDraft);
        let cases: [(&str, Set); 6] = [
            ("title", |d| d.title = "  ".into()),
            ("author", |d| d.author = String::new()),
            ("game", |d| d.game = "\t".into()),
            ("category", |d| d.category = " ".into()),
            ("description", |d| d.description = String::new()),
            ("version", |d| d.version = " ".into()),
        ];
        for (field, set) in cases {
            let mut d = draft();
            set(&mut d);
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::Blank { field }),
                "{field}"
            );
        }
    }

    #[test]
    fn id_must_be_a_slug() {
        for bad in ["", "Upper", "a b", "-a", "a-", "a--b", "a_b", "é"] {
            let mut d = draft();
            d.id = bad.into();
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::InvalidId),
                "{bad:?}"
            );
        }
        let mut long = draft();
        long.id = "a".repeat(65);
        assert_eq!(
            CatalogEntry::from_draft(long),
            Err(CatalogEntryError::InvalidId)
        );
        let mut max = draft();
        max.id = "a".repeat(64);
        assert!(CatalogEntry::from_draft(max).is_ok());
        let mut digits = draft();
        digits.id = "mod-2-x9".into();
        assert!(CatalogEntry::from_draft(digits).is_ok());
    }

    #[test]
    fn dates_must_be_real_calendar_days() {
        for bad in [
            "",
            "2026-1-01",
            "2026/11/20",
            "20261120",
            "2026-13-01",
            "2026-00-10",
            "2026-11-31",
            "2026-02-29",
            "2026-11-00",
            "abcd-ef-gh",
            "2026-11-2x",
            "+026-11-20",
        ] {
            let mut d = draft();
            d.updated_at = bad.into();
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::InvalidDate),
                "{bad:?}"
            );
        }
        for good in [
            "2024-02-29",
            "2000-02-29",
            "2026-02-28",
            "2026-12-31",
            "2026-04-30",
        ] {
            let mut d = draft();
            d.updated_at = good.into();
            assert!(CatalogEntry::from_draft(d).is_ok(), "{good}");
        }
        let mut century = draft();
        century.updated_at = "1900-02-29".into();
        assert_eq!(
            CatalogEntry::from_draft(century),
            Err(CatalogEntryError::InvalidDate)
        );
    }

    #[test]
    fn urls_must_be_https_with_a_host() {
        for bad in [
            "http://example.com",
            "file:///C:/x",
            "ftp://x.com",
            "example.com",
            "https://",
            "https:///path",
            "https://user@example.com/x",
            "https://user:pw@example.com",
            "https://exa mple.com",
            "https://example.com/a b",
            "",
            "  ",
            "https:/",
            "https://?q=1",
            "ttps://éééé",
        ] {
            let mut d = draft();
            d.page_url = bad.into();
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::InsecureUrl { field: "pageUrl" }),
                "{bad:?}"
            );
        }
        for good in [
            "HTTPS://Example.com/x",
            "https://example.com",
            "https://a.b:8080/x?y=1#z",
        ] {
            let mut d = draft();
            d.page_url = good.into();
            assert!(CatalogEntry::from_draft(d).is_ok(), "{good}");
        }
    }

    #[test]
    fn thumbnail_and_download_urls_name_their_field() {
        let mut d = draft();
        d.thumbnail_url = Some("http://example.com/t.png".into());
        assert_eq!(
            CatalogEntry::from_draft(d),
            Err(CatalogEntryError::InsecureUrl {
                field: "thumbnailUrl"
            })
        );
        let mut d = draft();
        d.download = download("http://example.com/f.zip", Some(1), Some(SHA));
        assert_eq!(
            CatalogEntry::from_draft(d),
            Err(CatalogEntryError::InsecureUrl {
                field: "download.url"
            })
        );
    }

    #[test]
    fn download_needs_size_and_sha256_together() {
        for (size, sha) in [(None, Some(SHA)), (Some(1), None), (None, None)] {
            let mut d = draft();
            d.download = download("https://example.com/f.zip", size, sha);
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::IncompleteDownload)
            );
        }
    }

    #[test]
    fn download_rejects_zero_size() {
        let mut d = draft();
        d.download = download("https://example.com/f.zip", Some(0), Some(SHA));
        assert_eq!(
            CatalogEntry::from_draft(d),
            Err(CatalogEntryError::InvalidSize)
        );
    }

    #[test]
    fn download_rejects_bad_checksums() {
        let upper = SHA.to_uppercase();
        let short = "0123456789";
        let non_hex = "g".repeat(64);
        let long = format!("{SHA}0");
        for bad in [upper.as_str(), short, non_hex.as_str(), long.as_str(), ""] {
            let mut d = draft();
            d.download = download("https://example.com/f.zip", Some(1), Some(bad));
            assert_eq!(
                CatalogEntry::from_draft(d),
                Err(CatalogEntryError::InvalidSha256),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn errors_have_readable_messages() {
        let all = [
            CatalogEntryError::Blank { field: "title" },
            CatalogEntryError::InvalidId,
            CatalogEntryError::InsecureUrl { field: "pageUrl" },
            CatalogEntryError::InvalidDate,
            CatalogEntryError::InvalidSha256,
            CatalogEntryError::InvalidSize,
            CatalogEntryError::IncompleteDownload,
        ];
        let text: Vec<String> = all.iter().map(|e| e.to_string()).collect();
        assert!(text[0].contains("title") && text[2].contains("pageUrl"));
        assert!(text.iter().all(|t| !t.is_empty()));
        let _: &dyn std::error::Error = &all[0];
    }

    #[test]
    fn supported_schema_is_one() {
        assert_eq!(SUPPORTED_CATALOG_SCHEMA, 1);
    }
}
