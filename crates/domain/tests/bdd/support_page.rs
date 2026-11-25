//! Steps for `tests/features/support-page.feature` (issue #94).
use cucumber::writer::Stats as _;
use cucumber::{given, then, World};
use motogp_domain::{check_page_url, check_support_url, PageUrlError, SUPPORT_PAGE_URL};

#[derive(Debug, Default, World)]
struct SupportWorld {
    url: String,
}

#[given("the configured support URL")]
fn configured(w: &mut SupportWorld) {
    w.url = SUPPORT_PAGE_URL.to_string();
}

#[given(expr = "the support URL {string}")]
fn support_url(w: &mut SupportWorld, url: String) {
    w.url = url;
}

#[then("the support URL is allowed")]
fn allowed(w: &mut SupportWorld) {
    assert_eq!(check_support_url(&w.url), Ok(()));
}

#[then("the support URL is rejected")]
fn rejected(w: &mut SupportWorld) {
    assert!(check_support_url(&w.url).is_err(), "expected a rejection");
}

#[then("the catalog policy rejects it as host not allowed")]
fn catalog_rejects(w: &mut SupportWorld) {
    assert!(matches!(
        check_page_url(&w.url),
        Err(PageUrlError::HostNotAllowed { .. })
    ));
}

/// Runs only `support-page.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = SupportWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("support-page.feature"))
        .await;
    !writer.execution_has_failed()
}
