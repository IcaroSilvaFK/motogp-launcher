//! Steps for `tests/features/support-page.feature` (issue #94), with an
//! in-memory web opener. Traceability: docs/validation-workflow.md.
use crate::catalog_fakes::FakeWebOpener;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{open_support_page, SupportPageError};
use motogp_domain::SUPPORT_PAGE_URL;

#[derive(Default, World)]
struct SupportWorld {
    opener: FakeWebOpener,
    result: Option<Result<(), SupportPageError>>,
}

impl std::fmt::Debug for SupportWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SupportWorld")
    }
}

#[given("the opener fails")]
fn opener_fails(w: &mut SupportWorld) {
    w.opener = FakeWebOpener::failing();
}

#[when("I open the support page")]
fn open(w: &mut SupportWorld) {
    w.result = Some(open_support_page(&w.opener));
}

#[then("the opener receives the configured support URL")]
fn received(w: &mut SupportWorld) {
    assert_eq!(w.result, Some(Ok(())));
    assert_eq!(w.opener.opened(), vec![SUPPORT_PAGE_URL.to_string()]);
}

#[then(expr = "the error is {string}")]
fn error_is(w: &mut SupportWorld, kind: String) {
    let error = w
        .result
        .as_ref()
        .expect("page not opened")
        .as_ref()
        .expect_err("expected a failure");
    let ok = match kind.as_str() {
        "not allowed" => matches!(error, SupportPageError::NotAllowed { .. }),
        "open failed" => matches!(error, SupportPageError::OpenFailed { .. }),
        other => panic!("unknown error kind {other}"),
    };
    assert!(ok, "got: {error:?}");
}

/// Runs only `support-page.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = SupportWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("support-page.feature"))
        .await;
    !writer.execution_has_failed()
}
