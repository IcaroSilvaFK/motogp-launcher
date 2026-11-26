//! Sponsored creatives (issue #101). Pure validation, no I/O.
//!
//! The ad feed is a self-hosted JSON file. Adapters parse it into
//! [`AdCreativeDraft`]s (unvalidated fields) and the rules here turn each draft
//! into a trusted [`AdCreative`] or reject it.

use crate::catalog::well_formed_https_url;
use std::collections::BTreeMap;
use std::fmt;

const MAX_ID_LEN: usize = 64;
const MAX_TITLE_CHARS: usize = 80;
const MAX_BODY_CHARS: usize = 200;
const MAX_SPONSOR_CHARS: usize = 40;

/// Where in the launcher a creative can appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdSlot {
    Discover,
    Sidebar,
    About,
}

impl AdSlot {
    /// Exact lowercase name: `discover`, `sidebar` or `about`.
    pub fn parse(value: &str) -> Option<AdSlot> {
        match value {
            "discover" => Some(AdSlot::Discover),
            "sidebar" => Some(AdSlot::Sidebar),
            "about" => Some(AdSlot::About),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AdSlot::Discover => "discover",
            AdSlot::Sidebar => "sidebar",
            AdSlot::About => "about",
        }
    }
}

/// Localized text overrides of one creative for one locale. Missing fields
/// fall back to the creative's default text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdLocaleText {
    pub title: Option<String>,
    pub body: Option<String>,
    pub sponsor_label: Option<String>,
}

/// Locales a creative may carry text for.
pub const AD_LOCALES: [&str; 2] = ["pt-BR", "en"];

/// Parsed feed fields, not yet validated. The adapter fills `image_data_uri`
/// after downloading the image and converts the expiry to Unix milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdCreativeDraft {
    pub id: String,
    pub slot: String,
    pub title: String,
    pub body: String,
    pub image_data_uri: Option<String>,
    pub click_url: String,
    pub sponsor_label: String,
    pub impression_url: Option<String>,
    pub expires_at_ms: Option<u64>,
    /// Localized text by locale (`pt-BR`, `en`).
    pub locales: BTreeMap<String, AdLocaleText>,
}

/// A validated creative: trimmed text, https URLs, a known slot and a safe
/// raster image (or none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdCreative {
    id: String,
    slot: AdSlot,
    title: String,
    body: String,
    image_data_uri: Option<String>,
    click_url: String,
    sponsor_label: String,
    impression_url: Option<String>,
    expires_at_ms: Option<u64>,
    locales: BTreeMap<String, AdLocaleText>,
}

/// The text of a creative shown in one locale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedAdText {
    pub title: String,
    pub body: String,
    pub sponsor_label: String,
}

/// Why an [`AdCreativeDraft`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdCreativeError {
    /// The id is empty, longer than 64 characters or not `[a-z0-9-]`.
    InvalidId,
    /// The slot is not one of the known slots.
    UnknownSlot(String),
    /// A required text field is empty or whitespace only.
    MissingField(&'static str),
    /// A text field exceeds its maximum length in characters.
    TooLong { field: &'static str, max: usize },
    /// A URL is not a well-formed `https://` URL.
    NotHttps(&'static str),
    /// The image is not a base64 `data:` URI of a PNG, JPEG or WebP (SVG is refused).
    InvalidImage,
    /// A `locales` key is not one of the supported locales.
    UnknownLocale(String),
}

impl fmt::Display for AdCreativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdCreativeError::InvalidId => write!(
                f,
                "id must be 1 to {MAX_ID_LEN} characters of lowercase letters, digits and dashes"
            ),
            AdCreativeError::UnknownSlot(slot) => write!(f, "unknown slot {slot:?}"),
            AdCreativeError::MissingField(field) => write!(f, "{field} is missing"),
            AdCreativeError::TooLong { field, max } => {
                write!(f, "{field} is longer than {max} characters")
            }
            AdCreativeError::NotHttps(field) => write!(f, "{field} must be a valid https URL"),
            AdCreativeError::UnknownLocale(locale) => write!(f, "unknown locale {locale:?}"),
            AdCreativeError::InvalidImage => {
                write!(f, "image must be a base64 PNG, JPEG or WebP data URI")
            }
        }
    }
}

impl std::error::Error for AdCreativeError {}

impl AdCreative {
    /// Validates a draft. The first broken rule wins.
    pub fn from_draft(draft: AdCreativeDraft) -> Result<AdCreative, AdCreativeError> {
        let id = draft.id.trim().to_string();
        if !is_valid_id(&id) {
            return Err(AdCreativeError::InvalidId);
        }
        let slot_text = draft.slot.trim();
        let slot = AdSlot::parse(slot_text)
            .ok_or_else(|| AdCreativeError::UnknownSlot(slot_text.to_string()))?;
        let title = text("title", &draft.title, MAX_TITLE_CHARS, true)?;
        let body = text("body", &draft.body, MAX_BODY_CHARS, false)?;
        let sponsor_label = text(
            "sponsorLabel",
            &draft.sponsor_label,
            MAX_SPONSOR_CHARS,
            true,
        )?;
        let click_url =
            well_formed_https_url(&draft.click_url).ok_or(AdCreativeError::NotHttps("clickUrl"))?;
        let impression_url = draft
            .impression_url
            .as_deref()
            .map(|url| well_formed_https_url(url).ok_or(AdCreativeError::NotHttps("impressionUrl")))
            .transpose()?;
        let image_data_uri = draft
            .image_data_uri
            .map(|image| {
                let image = image.trim().to_string();
                if is_raster_data_uri(&image) {
                    Ok(image)
                } else {
                    Err(AdCreativeError::InvalidImage)
                }
            })
            .transpose()?;
        let locales = draft
            .locales
            .into_iter()
            .map(|(locale, text)| validate_locale(locale, text))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(AdCreative {
            id,
            slot,
            title,
            body,
            image_data_uri,
            click_url,
            sponsor_label,
            impression_url,
            expires_at_ms: draft.expires_at_ms,
            locales,
        })
    }

    /// The text for `locale` (`pt-BR`, `pt`, `en-US`...): an exact match wins,
    /// then the same language; every missing field falls back to the default.
    pub fn localized(&self, locale: Option<&str>) -> LocalizedAdText {
        let entry = locale.and_then(|wanted| {
            let wanted = wanted.trim();
            let language = |tag: &str| tag.split(['-', '_']).next().unwrap_or("").to_lowercase();
            self.locales
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(wanted))
                .or_else(|| {
                    self.locales
                        .iter()
                        .find(|(key, _)| language(key) == language(wanted))
                })
                .map(|(_, text)| text)
        });
        let pick = |over: Option<&String>, default: &String| {
            over.cloned().unwrap_or_else(|| default.clone())
        };
        LocalizedAdText {
            title: pick(entry.and_then(|e| e.title.as_ref()), &self.title),
            body: pick(entry.and_then(|e| e.body.as_ref()), &self.body),
            sponsor_label: pick(
                entry.and_then(|e| e.sponsor_label.as_ref()),
                &self.sponsor_label,
            ),
        }
    }

    /// `true` when the creative expires at or before `now_ms`.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.expires_at_ms.is_some_and(|at| at <= now_ms)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn slot(&self) -> AdSlot {
        self.slot
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn image_data_uri(&self) -> Option<&str> {
        self.image_data_uri.as_deref()
    }
    pub fn click_url(&self) -> &str {
        &self.click_url
    }
    pub fn sponsor_label(&self) -> &str {
        &self.sponsor_label
    }
    pub fn impression_url(&self) -> Option<&str> {
        self.impression_url.as_deref()
    }
    pub fn locales(&self) -> &BTreeMap<String, AdLocaleText> {
        &self.locales
    }
    pub fn expires_at_ms(&self) -> Option<u64> {
        self.expires_at_ms
    }
}

fn validate_locale(
    locale: String,
    given: AdLocaleText,
) -> Result<(String, AdLocaleText), AdCreativeError> {
    if !AD_LOCALES.contains(&locale.as_str()) {
        return Err(AdCreativeError::UnknownLocale(locale));
    }
    let field = |name: &'static str, value: Option<String>, max: usize| {
        value.map(|v| text(name, &v, max, true)).transpose()
    };
    let validated = AdLocaleText {
        title: field("title", given.title, MAX_TITLE_CHARS)?,
        body: field("body", given.body, MAX_BODY_CHARS)?,
        sponsor_label: field("sponsorLabel", given.sponsor_label, MAX_SPONSOR_CHARS)?,
    };
    Ok((locale, validated))
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Trims `value` and checks it is present (when `required`) and not too long.
fn text(
    field: &'static str,
    value: &str,
    max: usize,
    required: bool,
) -> Result<String, AdCreativeError> {
    let trimmed = value.trim();
    if required && trimmed.is_empty() {
        return Err(AdCreativeError::MissingField(field));
    }
    if trimmed.chars().count() > max {
        return Err(AdCreativeError::TooLong { field, max });
    }
    Ok(trimmed.to_string())
}

/// `data:image/(png|jpeg|webp);base64,<non-empty base64>`.
fn is_raster_data_uri(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("data:image/") else {
        return false;
    };
    let Some((mime, payload)) = rest.split_once(";base64,") else {
        return false;
    };
    matches!(mime, "png" | "jpeg" | "webp")
        && !payload.is_empty()
        && payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> AdCreativeDraft {
        AdCreativeDraft {
            id: "acme-1".into(),
            slot: "discover".into(),
            title: "Acme tires".into(),
            body: "Grip for every lap".into(),
            image_data_uri: None,
            click_url: "https://example.com/acme".into(),
            sponsor_label: "Acme Racing".into(),
            impression_url: None,
            expires_at_ms: None,
            ..Default::default()
        }
    }

    #[test]
    fn slots_parse_exact_lowercase_names() {
        for slot in [AdSlot::Discover, AdSlot::Sidebar, AdSlot::About] {
            assert_eq!(AdSlot::parse(slot.as_str()), Some(slot));
        }
        for bad in ["Discover", "popup", "", " about"] {
            assert_eq!(AdSlot::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn complete_draft_is_valid_and_trimmed() {
        let mut d = draft();
        d.title = "  Acme tires  ".into();
        d.body = "".into();
        d.slot = " sidebar ".into();
        d.impression_url = Some(" https://example.com/i ".into());
        d.image_data_uri = Some(" data:image/png;base64,iVBORw0KGgo= ".into());
        d.expires_at_ms = Some(10);
        let ad = AdCreative::from_draft(d).unwrap();
        assert_eq!(ad.id(), "acme-1");
        assert_eq!(ad.slot(), AdSlot::Sidebar);
        assert_eq!(ad.title(), "Acme tires");
        assert_eq!(ad.body(), "");
        assert_eq!(ad.sponsor_label(), "Acme Racing");
        assert_eq!(ad.click_url(), "https://example.com/acme");
        assert_eq!(ad.impression_url(), Some("https://example.com/i"));
        assert_eq!(
            ad.image_data_uri(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        assert_eq!(ad.expires_at_ms(), Some(10));
        assert_eq!(ad.body(), "");
    }

    #[test]
    fn optional_fields_may_be_absent() {
        let ad = AdCreative::from_draft(draft()).unwrap();
        assert!(ad.image_data_uri().is_none() && ad.impression_url().is_none());
        assert!(ad.expires_at_ms().is_none());
    }

    #[test]
    fn id_must_be_a_short_slug() {
        for bad in ["", "Upper", "a b", "a_b", "é"] {
            let mut d = draft();
            d.id = bad.into();
            assert_eq!(
                AdCreative::from_draft(d),
                Err(AdCreativeError::InvalidId),
                "{bad:?}"
            );
        }
        let mut long = draft();
        long.id = "a".repeat(65);
        assert_eq!(
            AdCreative::from_draft(long),
            Err(AdCreativeError::InvalidId)
        );
        let mut max = draft();
        max.id = "a".repeat(64);
        assert!(AdCreative::from_draft(max).is_ok());
    }

    #[test]
    fn unknown_slot_is_named() {
        let mut d = draft();
        d.slot = "popup".into();
        assert_eq!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::UnknownSlot("popup".into()))
        );
    }

    #[test]
    fn blank_title_and_sponsor_label_are_missing() {
        let mut d = draft();
        d.title = "  ".into();
        assert_eq!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::MissingField("title"))
        );
        let mut d = draft();
        d.sponsor_label = "   ".into();
        assert_eq!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::MissingField("sponsorLabel"))
        );
    }

    #[test]
    fn text_lengths_are_limited_in_characters() {
        type Set = fn(&mut AdCreativeDraft, String);
        let cases: [(&str, usize, Set); 3] = [
            ("title", 80, |d, v| d.title = v),
            ("body", 200, |d, v| d.body = v),
            ("sponsorLabel", 40, |d, v| d.sponsor_label = v),
        ];
        for (field, max, set) in cases {
            let mut over = draft();
            set(&mut over, "é".repeat(max + 1));
            assert_eq!(
                AdCreative::from_draft(over),
                Err(AdCreativeError::TooLong { field, max })
            );
            let mut at_max = draft();
            set(&mut at_max, "é".repeat(max));
            assert!(AdCreative::from_draft(at_max).is_ok(), "{field}");
        }
    }

    #[test]
    fn urls_must_be_https() {
        for bad in ["http://example.com", "", "https://", "https://u@x.com"] {
            let mut d = draft();
            d.click_url = bad.into();
            assert_eq!(
                AdCreative::from_draft(d),
                Err(AdCreativeError::NotHttps("clickUrl")),
                "{bad:?}"
            );
            let mut d = draft();
            d.impression_url = Some(bad.into());
            assert_eq!(
                AdCreative::from_draft(d),
                Err(AdCreativeError::NotHttps("impressionUrl")),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn only_raster_base64_data_uris_are_images() {
        for good in [
            "data:image/png;base64,AAAA",
            "data:image/jpeg;base64,/9j/4AAQ+=",
            "data:image/webp;base64,UklGRg==",
        ] {
            let mut d = draft();
            d.image_data_uri = Some(good.into());
            assert!(AdCreative::from_draft(d).is_ok(), "{good}");
        }
        for bad in [
            "data:image/svg+xml;base64,PHN2Zy8+",
            "data:image/gif;base64,AAAA",
            "data:text/html;base64,AAAA",
            "data:image/png,AAAA",
            "data:image/png;base64,",
            "data:image/png;base64,AA AA",
            "data:image/png;base64,<script>",
            "https://example.com/a.png",
            "",
        ] {
            let mut d = draft();
            d.image_data_uri = Some(bad.into());
            assert_eq!(
                AdCreative::from_draft(d),
                Err(AdCreativeError::InvalidImage),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn expiry_is_inclusive_and_optional() {
        let mut d = draft();
        d.expires_at_ms = Some(100);
        let ad = AdCreative::from_draft(d).unwrap();
        assert!(!ad.is_expired(99));
        assert!(ad.is_expired(100));
        assert!(ad.is_expired(101));
        assert!(!AdCreative::from_draft(draft())
            .unwrap()
            .is_expired(u64::MAX));
    }

    fn with_pt(title: Option<&str>) -> AdCreative {
        let mut d = draft();
        d.locales.insert(
            "pt-BR".into(),
            AdLocaleText {
                title: title.map(String::from),
                body: Some(" Corpo ".into()),
                sponsor_label: None,
            },
        );
        AdCreative::from_draft(d).unwrap()
    }

    #[test]
    fn localized_text_falls_back_to_the_defaults() {
        let ad = with_pt(Some("Titulo"));
        for wanted in [Some("pt-BR"), Some("pt-br"), Some("pt")] {
            let text = ad.localized(wanted);
            assert_eq!(
                (text.title.as_str(), text.body.as_str()),
                ("Titulo", "Corpo")
            );
            assert_eq!(text.sponsor_label, "Acme Racing");
        }
        for wanted in [None, Some("en"), Some("fr")] {
            let text = ad.localized(wanted);
            assert_eq!(
                (text.title.as_str(), text.body.as_str()),
                ("Acme tires", "Grip for every lap")
            );
        }
    }

    #[test]
    fn only_known_locales_with_valid_text_are_accepted() {
        let mut d = draft();
        d.locales.insert("fr".into(), AdLocaleText::default());
        assert_eq!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::UnknownLocale("fr".into()))
        );
        let mut d = draft();
        d.locales.insert(
            "en".into(),
            AdLocaleText {
                title: Some("  ".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::MissingField("title"))
        );
        let mut d = draft();
        d.locales.insert(
            "en".into(),
            AdLocaleText {
                body: Some("x".repeat(MAX_BODY_CHARS + 1)),
                ..Default::default()
            },
        );
        assert!(matches!(
            AdCreative::from_draft(d),
            Err(AdCreativeError::TooLong { field: "body", .. })
        ));
    }
    #[test]
    fn errors_have_readable_messages() {
        let all = [
            AdCreativeError::InvalidId,
            AdCreativeError::UnknownSlot("popup".into()),
            AdCreativeError::MissingField("title"),
            AdCreativeError::TooLong {
                field: "body",
                max: 200,
            },
            AdCreativeError::NotHttps("clickUrl"),
            AdCreativeError::InvalidImage,
        ];
        let text: Vec<String> = all.iter().map(|e| e.to_string()).collect();
        assert!(text[1].contains("popup") && text[2].contains("title"));
        assert!(text[3].contains("200") && text[4].contains("clickUrl"));
        assert!(text.iter().all(|t| !t.is_empty()));
        let _: &dyn std::error::Error = &all[0];
    }
}
