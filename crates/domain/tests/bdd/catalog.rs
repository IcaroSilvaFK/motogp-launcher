//! Steps for `tests/features/catalog.feature` (issue #72). Pure validation rules.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_domain::{CatalogDownloadDraft, CatalogEntry, CatalogEntryDraft, CatalogEntryError};

#[derive(Debug, Default, World)]
struct CatalogWorld {
    draft: Option<CatalogEntryDraft>,
    result: Option<Result<CatalogEntry, CatalogEntryError>>,
}

fn complete_draft() -> CatalogEntryDraft {
    CatalogEntryDraft {
        id: "rebalanced-ai".into(),
        title: "Rebalanced AI".into(),
        author: "Some Author".into(),
        game: "MotoGP 26".into(),
        category: "Gameplay".into(),
        description: "Tweaks the rider AI.".into(),
        version: "1.2.0".into(),
        updated_at: "2026-11-20".into(),
        page_url: "https://example.com/mods/rebalanced-ai".into(),
        thumbnail_url: None,
        screenshots: Vec::new(),
        long_description: None,
        download: None,
    }
}

impl CatalogWorld {
    fn draft_mut(&mut self) -> &mut CatalogEntryDraft {
        self.draft.as_mut().expect("no draft")
    }
}

#[given("a draft with every required field and an https page URL")]
fn complete(w: &mut CatalogWorld) {
    w.draft = Some(complete_draft());
}

#[given(expr = "a draft whose page URL is {string}")]
fn page_url(w: &mut CatalogWorld, url: String) {
    let mut draft = complete_draft();
    draft.page_url = url;
    w.draft = Some(draft);
}

#[given("a draft whose download has a size but a 10-character sha256")]
fn short_checksum(w: &mut CatalogWorld) {
    let mut draft = complete_draft();
    draft.download = Some(CatalogDownloadDraft {
        url: "https://example.com/files/rebalanced-ai.zip".into(),
        size: Some(1024),
        sha256: Some("0123456789".into()),
    });
    w.draft = Some(draft);
}

#[given(expr = "a draft whose title is {string}")]
fn blank_title(w: &mut CatalogWorld, title: String) {
    let mut draft = complete_draft();
    draft.title = title;
    w.draft = Some(draft);
}

fn shots(count: usize) -> Vec<String> {
    (1..=count)
        .map(|i| format!("https://example.com/shot-{i}.png"))
        .collect()
}

#[given(expr = "a catalog entry with {int} https screenshots and a long description")]
fn with_screenshots(w: &mut CatalogWorld, count: usize) {
    let mut draft = complete_draft();
    draft.screenshots = shots(count);
    draft.long_description = Some("A longer text.".into());
    w.draft = Some(draft);
}

#[given("a catalog entry without screenshots or long description")]
fn without_screenshots(w: &mut CatalogWorld) {
    w.draft = Some(complete_draft());
}

#[given(expr = "a catalog entry with the screenshot {string}")]
fn one_screenshot(w: &mut CatalogWorld, url: String) {
    let mut draft = complete_draft();
    draft.screenshots = vec![url];
    w.draft = Some(draft);
}

#[given(expr = "a catalog entry with {int} screenshots")]
fn many_screenshots(w: &mut CatalogWorld, count: usize) {
    let mut draft = complete_draft();
    draft.screenshots = shots(count);
    w.draft = Some(draft);
}

#[when("it is validated")]
fn validate(w: &mut CatalogWorld) {
    let draft = w.draft_mut().clone();
    w.result = Some(CatalogEntry::from_draft(draft));
}

fn result(w: &CatalogWorld) -> &Result<CatalogEntry, CatalogEntryError> {
    w.result.as_ref().expect("not validated")
}

#[then("a catalog entry is produced")]
fn produced(w: &mut CatalogWorld) {
    let entry = result(w).as_ref().expect("expected a valid entry");
    assert_eq!(entry.id, "rebalanced-ai");
}

#[then(expr = "it is rejected with an {string} reason")]
fn rejected_with(w: &mut CatalogWorld, reason: String) {
    let error = result(w).as_ref().expect_err("expected a rejection");
    match reason.as_str() {
        "insecure URL" => assert!(
            matches!(error, CatalogEntryError::InsecureUrl { .. }),
            "got: {error:?}"
        ),
        other => panic!("unknown reason {other}"),
    }
}

#[then(expr = "it keeps the {int} screenshots in order and the long description")]
fn keeps_screenshots(w: &mut CatalogWorld, count: usize) {
    let entry = result(w).as_ref().expect("expected a valid entry");
    assert_eq!(entry.screenshots, shots(count));
    assert_eq!(entry.long_description.as_deref(), Some("A longer text."));
}

#[then("it has no screenshots and no long description")]
fn has_none(w: &mut CatalogWorld) {
    let entry = result(w).as_ref().expect("expected a valid entry");
    assert!(entry.screenshots.is_empty());
    assert_eq!(entry.long_description, None);
}

#[then(expr = "it is refused naming the field {string}")]
fn refused_naming(w: &mut CatalogWorld, field: String) {
    let error = result(w).as_ref().expect_err("expected a rejection");
    assert!(
        matches!(error, CatalogEntryError::InsecureUrl { field: f } if *f == field),
        "got: {error:?}"
    );
}

#[then("it is refused because at most 8 screenshots are allowed")]
fn refused_too_many(w: &mut CatalogWorld) {
    let error = result(w).as_ref().expect_err("expected a rejection");
    assert_eq!(error, &CatalogEntryError::TooManyScreenshots { max: 8 });
}

#[then("it is rejected")]
fn rejected(w: &mut CatalogWorld) {
    assert!(result(w).is_err(), "expected a rejection");
}

/// Runs only `catalog.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CatalogWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("catalog.feature"))
        .await;
    !writer.execution_has_failed()
}
