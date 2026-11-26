//! Steps for `tests/features/ad-creative.feature` (issue #101).
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{AdCreative, AdCreativeDraft, AdCreativeError};

#[derive(Debug, Default, World)]
struct AdWorld {
    draft: Option<AdCreativeDraft>,
    result: Option<Result<AdCreative, AdCreativeError>>,
}

fn base() -> AdCreativeDraft {
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
    }
}

#[given(
    expr = "a creative draft for the {string} slot with an https click URL and the sponsor label {string}"
)]
fn complete(w: &mut AdWorld, slot: String, label: String) {
    w.draft = Some(AdCreativeDraft {
        slot,
        sponsor_label: label,
        ..base()
    });
}

#[given(expr = "a creative draft whose click URL is {string}")]
fn click_url(w: &mut AdWorld, url: String) {
    w.draft = Some(AdCreativeDraft {
        click_url: url,
        ..base()
    });
}

#[given(expr = "a creative draft whose sponsor label is {string}")]
fn blank_label(w: &mut AdWorld, text: String) {
    w.draft = Some(AdCreativeDraft {
        sponsor_label: text,
        ..base()
    });
}

#[given(expr = "a creative draft for the {string} slot")]
fn other_slot(w: &mut AdWorld, name: String) {
    w.draft = Some(AdCreativeDraft {
        slot: name,
        ..base()
    });
}

#[given(expr = "a creative draft whose image is {string}")]
fn bad_image(w: &mut AdWorld, uri: String) {
    w.draft = Some(AdCreativeDraft {
        image_data_uri: Some(uri),
        ..base()
    });
}

#[when("it is validated")]
fn validate(w: &mut AdWorld) {
    w.result = Some(AdCreative::from_draft(w.draft.clone().unwrap()));
}

#[then(expr = "it is accepted with the slot {string}")]
fn accepted(w: &mut AdWorld, slot: String) {
    let ad = w.result.take().unwrap().expect("expected acceptance");
    assert_eq!(ad.slot().as_str(), slot);
}

#[then(expr = "it is rejected because {string} is not https")]
fn not_https(w: &mut AdWorld, field: String) {
    let error = w.result.take().unwrap().unwrap_err();
    assert!(matches!(error, AdCreativeError::NotHttps(f) if f == field));
}

#[then(expr = "it is rejected because {string} is missing")]
fn missing(w: &mut AdWorld, field: String) {
    let error = w.result.take().unwrap().unwrap_err();
    assert!(matches!(error, AdCreativeError::MissingField(f) if f == field));
}

#[then("it is rejected because the slot is unknown")]
fn unknown_slot(w: &mut AdWorld) {
    let error = w.result.take().unwrap().unwrap_err();
    assert!(matches!(error, AdCreativeError::UnknownSlot(_)));
}

#[then("it is rejected because the image is invalid")]
fn invalid_image(w: &mut AdWorld) {
    let error = w.result.take().unwrap().unwrap_err();
    assert_eq!(error, AdCreativeError::InvalidImage);
}

/// Runs only `ad-creative.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = AdWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("ad-creative.feature"))
        .await;
    !writer.execution_has_failed()
}
