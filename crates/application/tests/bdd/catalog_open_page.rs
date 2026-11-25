//! Steps for `tests/features/catalog-open-page.feature` (issue #73), with
//! in-memory fakes. Traceability: docs/validation-workflow.md.
use crate::catalog_fakes::*;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::CatalogError;

#[derive(Default, World)]
struct OpenPageWorld {
    cache: FakeCache,
    opener: FakeWebOpener,
    result: Option<Result<(), CatalogError>>,
}

impl std::fmt::Debug for OpenPageWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpenPageWorld")
    }
}

#[given(expr = "the catalog holds the entry {string} with page {string}")]
fn holds(w: &mut OpenPageWorld, id: String, page: String) {
    w.cache = FakeCache::holding(cached_with_page(&id, &page));
}

#[given("the opener fails")]
fn opener_fails(w: &mut OpenPageWorld) {
    w.opener = FakeWebOpener::failing();
}

#[when(expr = "I open the page of {string}")]
fn open(w: &mut OpenPageWorld, id: String) {
    // The source is unreachable: opening must work from the cache alone.
    let source = FakeSource::failing(motogp_application::CatalogSourceError::Network(
        "down".into(),
    ));
    let svc = service(&source, &w.cache, &FakeClock::at(NOW_MS));
    w.result = Some(svc.open_page(&id, &w.opener));
    assert_eq!(source.calls(), 0, "opening a page must not fetch");
}

#[then(expr = "the opener receives {string}")]
fn received(w: &mut OpenPageWorld, url: String) {
    assert_eq!(w.result, Some(Ok(())));
    assert_eq!(w.opener.opened(), vec![url]);
}

#[then(expr = "the error is {string} and nothing is opened")]
fn error_and_nothing(w: &mut OpenPageWorld, kind: String) {
    check_error(w, &kind);
    assert!(w.opener.opened().is_empty());
}

#[then(expr = "the error is {string}")]
fn error_is(w: &mut OpenPageWorld, kind: String) {
    check_error(w, &kind);
}

fn check_error(w: &OpenPageWorld, kind: &str) {
    let error = w
        .result
        .as_ref()
        .expect("page not opened")
        .as_ref()
        .expect_err("expected a failure");
    let ok = match kind {
        "entry not found" => matches!(error, CatalogError::EntryNotFound { .. }),
        "page not allowed" => matches!(error, CatalogError::PageNotAllowed { .. }),
        "open failed" => matches!(error, CatalogError::OpenFailed { .. }),
        other => panic!("unknown error kind {other}"),
    };
    assert!(ok, "got: {error:?}");
}

/// Runs only `catalog-open-page.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = OpenPageWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("catalog-open-page.feature"))
        .await;
    !writer.execution_has_failed()
}
