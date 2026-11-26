//! Mod hosting rules: slugs, metadata validation, file types, storage keys.
//! Plain types, no IO.

use chrono::{DateTime, Utc};

use crate::UserId;

/// Size of one upload chunk (also the body limit of the chunk route).
pub const CHUNK_SIZE: u64 = 8 * 1024 * 1024;
/// Largest accepted screenshot.
pub const MAX_SCREENSHOT_BYTES: u64 = 5 * 1024 * 1024;
/// Screenshots per version.
pub const MAX_SCREENSHOTS: usize = 8;
/// Longest `longDescription` (same cap as the catalog schema).
pub const MAX_LONG_DESCRIPTION_CHARS: usize = 20_000;
/// How long an upload session stays open.
pub const UPLOAD_LIFETIME_HOURS: i64 = 24;
/// Longest reject reason.
pub const MAX_REASON_CHARS: usize = 500;

const MAX_SLUG_LEN: usize = 64;
const MAX_TITLE_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 1_000;
const MAX_SHORT_FIELD_CHARS: usize = 64;
const MAX_VERSION_LEN: usize = 32;
const MAX_FILE_NAME_LEN: usize = 128;
const ALLOWED_EXTENSIONS: [&str; 3] = ["zip", "7z", "pak"];
/// Ids that would collide with fixed routes under `/mods/`.
const RESERVED_SLUGS: [&str; 3] = ["uploads", "mine", "stats"];

/// A rule that the input broke. The message is safe to show to clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModRuleError {
    /// A field is blank, too long or malformed.
    Invalid(String),
    /// The file extension or image type is not allowed.
    UnsupportedFile(String),
}

/// Mod id: lowercase letters and digits separated by single hyphens, at most 64 chars.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModSlug(String);

impl ModSlug {
    pub fn parse(value: &str) -> Option<Self> {
        let valid = !value.is_empty()
            && value.len() <= MAX_SLUG_LEN
            && !value.starts_with('-')
            && !value.ends_with('-')
            && !value.contains("--")
            && value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Review state of one version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionStatus {
    Pending,
    Approved,
    Rejected,
}

impl VersionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// An image type accepted for screenshots, detected from the first bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Webp,
}

impl ImageKind {
    /// Detects the type by magic bytes; the client's content type is never trusted.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else {
            None
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
        }
    }
}

/// Relative path of a stored file, built only by the server from validated
/// parts (`<mod>/<version>/<file>`). Never constructed from raw user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageKey(String);

impl StorageKey {
    pub fn file(slug: &ModSlug, version: &str, file_name: &str) -> Self {
        Self(format!("{}/{version}/{file_name}", slug.as_str()))
    }

    pub fn screenshot(slug: &ModSlug, version: &str, index: usize, kind: ImageKind) -> Self {
        Self(format!(
            "{}/{version}/screenshot-{index}.{}",
            slug.as_str(),
            kind.extension()
        ))
    }

    /// Rebuilds a key stored in the database (the adapter re-validates it).
    pub fn from_stored(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated metadata of an upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModMetadata {
    pub mod_id: ModSlug,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub version: String,
    pub file_name: String,
    pub size: u64,
    /// Lowercase hex SHA-256 the client computed, checked at completion.
    pub client_sha256: Option<String>,
}

/// Raw, unvalidated input of `POST /mods/uploads`.
#[derive(Clone, Debug, Default)]
pub struct RawMetadata {
    pub mod_id: String,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub version: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: Option<String>,
    pub ownership_confirmed: bool,
}

fn invalid(message: &str) -> ModRuleError {
    ModRuleError::Invalid(message.to_owned())
}

fn bounded_text(value: &str, name: &str, max: usize) -> Result<String, ModRuleError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ModRuleError::Invalid(format!("{name} is required.")));
    }
    if value.chars().count() > max {
        return Err(ModRuleError::Invalid(format!(
            "{name} must be at most {max} characters."
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(ModRuleError::Invalid(format!(
            "{name} has invalid characters."
        )));
    }
    Ok(value.to_owned())
}

/// A version number is also a folder name: letters, digits, `.`, `_`, `+`, `-`.
fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_VERSION_LEN
        && value != "."
        && value != ".."
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
}

/// A file name is also a path segment: no separators, no leading dot, safe characters only.
pub fn valid_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_FILE_NAME_LEN
        && !value.starts_with('.')
        && !value.contains("..")
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b' ' | b'(' | b')')
        })
}

/// Whether the name ends in an allowed extension (case-insensitive).
pub fn has_allowed_extension(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .map(|(_, ext)| ALLOWED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl ModMetadata {
    /// Validates the raw input. The size cap is a server setting, checked by the use case.
    pub fn validate(raw: RawMetadata) -> Result<Self, ModRuleError> {
        let mod_id = ModSlug::parse(&raw.mod_id).ok_or_else(|| {
            invalid(
                "The mod id must be lowercase letters and digits separated by hyphens (max 64).",
            )
        })?;
        if RESERVED_SLUGS.contains(&mod_id.as_str()) {
            return Err(invalid("This mod id is reserved."));
        }
        let title = bounded_text(&raw.title, "The title", MAX_TITLE_CHARS)?;
        let description = bounded_text(&raw.description, "The description", MAX_DESCRIPTION_CHARS)?;
        let category = bounded_text(&raw.category, "The category", MAX_SHORT_FIELD_CHARS)?;
        let game = bounded_text(&raw.game, "The game", MAX_SHORT_FIELD_CHARS)?;
        let long_description = match raw.long_description {
            Some(text) if !text.trim().is_empty() => {
                if text.chars().count() > MAX_LONG_DESCRIPTION_CHARS {
                    return Err(ModRuleError::Invalid(format!(
                        "The long description must be at most {MAX_LONG_DESCRIPTION_CHARS} characters."
                    )));
                }
                Some(text.trim().to_owned())
            }
            _ => None,
        };
        let version = raw.version.trim().to_owned();
        if !valid_version(&version) {
            return Err(invalid(
                "The version must be 1 to 32 letters, digits, dots, underscores, plus signs or hyphens.",
            ));
        }
        if !valid_file_name(&raw.file_name) {
            return Err(invalid("The file name is invalid."));
        }
        if !has_allowed_extension(&raw.file_name) {
            return Err(ModRuleError::UnsupportedFile(
                "Only .zip, .7z and .pak files are accepted.".into(),
            ));
        }
        if raw.size == 0 {
            return Err(invalid("The file size must be greater than zero."));
        }
        let client_sha256 = match raw.sha256 {
            Some(hash) => {
                let hash = hash.trim().to_ascii_lowercase();
                if !valid_sha256(&hash) {
                    return Err(invalid("The sha256 must be 64 hexadecimal characters."));
                }
                Some(hash)
            }
            None => None,
        };
        if !raw.ownership_confirmed {
            return Err(invalid(
                "You must confirm that you own or may redistribute this mod.",
            ));
        }
        Ok(Self {
            mod_id,
            title,
            description,
            long_description,
            category,
            game,
            version,
            file_name: raw.file_name,
            size: raw.size,
            client_sha256,
        })
    }
}

/// A resumable upload session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModUpload {
    pub id: String,
    pub user_id: UserId,
    pub meta: ModMetadata,
    pub received: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// One stored screenshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    pub index: usize,
    pub storage_key: StorageKey,
    pub content_type: String,
}

/// A hosted version of a mod (every version stays as history).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModVersion {
    pub id: String,
    pub mod_id: ModSlug,
    pub owner_user_id: UserId,
    pub version: String,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub storage_key: StorageKey,
    pub status: VersionStatus,
    pub rejection_reason: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub screenshots: Vec<Screenshot>,
}

/// The latest approved version of a mod plus its author's display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedMod {
    pub version: ModVersion,
    pub author: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw() -> RawMetadata {
        RawMetadata {
            mod_id: "brazil-livery".into(),
            title: "Brazil Livery".into(),
            description: "A livery".into(),
            long_description: None,
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            version: "1.0".into(),
            file_name: "Brazil Livery.zip".into(),
            size: 10,
            sha256: None,
            ownership_confirmed: true,
        }
    }

    #[test]
    fn slug_follows_the_catalog_rule() {
        for ok in ["a", "brazil-livery", "mod-2", "a1-b2-c3"] {
            assert!(ModSlug::parse(ok).is_some(), "{ok}");
        }
        for bad in ["", "-a", "a-", "a--b", "A", "a_b", "a b", "../x", "a/b"] {
            assert!(ModSlug::parse(bad).is_none(), "{bad}");
        }
        assert!(ModSlug::parse(&"a".repeat(64)).is_some());
        assert!(ModSlug::parse(&"a".repeat(65)).is_none());
    }

    #[test]
    fn status_round_trips() {
        for status in [
            VersionStatus::Pending,
            VersionStatus::Approved,
            VersionStatus::Rejected,
        ] {
            assert_eq!(VersionStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(VersionStatus::parse("other"), None);
    }

    #[test]
    fn extensions_are_case_insensitive_and_limited() {
        for ok in ["a.zip", "a.ZIP", "a.7z", "a.PaK", "x.y.zip"] {
            assert!(has_allowed_extension(ok), "{ok}");
        }
        for bad in ["a.exe", "a", "zip", "a.zip.exe", "a.rar", "a."] {
            assert!(!has_allowed_extension(bad), "{bad}");
        }
    }

    #[test]
    fn file_names_cannot_climb_or_contain_separators() {
        assert!(valid_file_name("Brazil Livery (v2).zip"));
        for bad in [
            "../a.zip",
            "a/b.zip",
            "a\\b.zip",
            ".hidden.zip",
            "a..zip",
            "",
            "caf\u{e9}.zip",
        ] {
            assert!(!valid_file_name(bad), "{bad}");
        }
        assert!(!valid_file_name(&format!("{}.zip", "a".repeat(130))));
    }

    #[test]
    fn valid_metadata_is_accepted_and_trimmed() {
        let mut input = raw();
        input.title = "  Brazil Livery  ".into();
        input.sha256 = Some("A".repeat(64));
        input.long_description = Some("  long text ".into());
        let meta = ModMetadata::validate(input).unwrap();
        assert_eq!(meta.title, "Brazil Livery");
        assert_eq!(meta.client_sha256, Some("a".repeat(64)));
        assert_eq!(meta.long_description.as_deref(), Some("long text"));
    }

    #[test]
    fn blank_long_description_becomes_none() {
        let mut input = raw();
        input.long_description = Some("   ".into());
        assert_eq!(ModMetadata::validate(input).unwrap().long_description, None);
    }

    #[test]
    fn invalid_fields_are_rejected() {
        type Mutation = Box<dyn Fn(&mut RawMetadata)>;
        let cases: Vec<Mutation> = vec![
            Box::new(|r| r.mod_id = "Bad Id".into()),
            Box::new(|r| r.mod_id = "mine".into()),
            Box::new(|r| r.mod_id = "uploads".into()),
            Box::new(|r| r.mod_id = "stats".into()),
            Box::new(|r| r.title = "   ".into()),
            Box::new(|r| r.title = "t".repeat(121)),
            Box::new(|r| r.description = String::new()),
            Box::new(|r| r.description = "d".repeat(1_001)),
            Box::new(|r| r.category = "c".repeat(65)),
            Box::new(|r| r.game = " ".into()),
            Box::new(|r| r.title = "a\u{0}b".into()),
            Box::new(|r| r.long_description = Some("x".repeat(20_001))),
            Box::new(|r| r.version = "../1".into()),
            Box::new(|r| r.version = "..".into()),
            Box::new(|r| r.version = String::new()),
            Box::new(|r| r.version = "v".repeat(33)),
            Box::new(|r| r.file_name = "../a.zip".into()),
            Box::new(|r| r.size = 0),
            Box::new(|r| r.sha256 = Some("xyz".into())),
            Box::new(|r| r.ownership_confirmed = false),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let mut input = raw();
            mutate(&mut input);
            assert!(
                matches!(ModMetadata::validate(input), Err(ModRuleError::Invalid(_))),
                "case {i}"
            );
        }
    }

    #[test]
    fn disallowed_extension_is_an_unsupported_file() {
        let mut input = raw();
        input.file_name = "setup.exe".into();
        assert!(matches!(
            ModMetadata::validate(input),
            Err(ModRuleError::UnsupportedFile(_))
        ));
    }

    #[test]
    fn images_are_detected_by_magic_bytes() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];
        assert_eq!(ImageKind::sniff(&png), Some(ImageKind::Png));
        assert_eq!(
            ImageKind::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(ImageKind::Jpeg)
        );
        assert_eq!(
            ImageKind::sniff(b"RIFF\x00\x00\x00\x00WEBPVP8 "),
            Some(ImageKind::Webp)
        );
        assert_eq!(ImageKind::sniff(b"GIF89a"), None);
        assert_eq!(ImageKind::sniff(b"RIFF\x00\x00\x00\x00WAVE"), None);
        assert_eq!(ImageKind::sniff(b""), None);
        assert_eq!(ImageKind::Png.extension(), "png");
        assert_eq!(ImageKind::Jpeg.extension(), "jpg");
        assert_eq!(ImageKind::Webp.content_type(), "image/webp");
        assert_eq!(ImageKind::Png.content_type(), "image/png");
        assert_eq!(ImageKind::Jpeg.content_type(), "image/jpeg");
        assert_eq!(ImageKind::Webp.extension(), "webp");
    }

    #[test]
    fn storage_keys_are_built_from_validated_parts() {
        let slug = ModSlug::parse("brazil-livery").unwrap();
        assert_eq!(
            StorageKey::file(&slug, "1.0", "a.zip").as_str(),
            "brazil-livery/1.0/a.zip"
        );
        assert_eq!(
            StorageKey::screenshot(&slug, "1.0", 2, ImageKind::Jpeg).as_str(),
            "brazil-livery/1.0/screenshot-2.jpg"
        );
        assert_eq!(StorageKey::from_stored("x/y/z".into()).as_str(), "x/y/z");
    }
}
