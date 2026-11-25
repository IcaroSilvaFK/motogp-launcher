//! Steps for `tests/features/catalog-page-url.feature` (issue #73).
use cucumber::writer::Stats as _;
use cucumber::{given, then, World};
use motogp_domain::check_page_url;

#[derive(Debug, Default, World)]
struct PageUrlWorld {
    url: String,
}

#[given(expr = "the page URL {string}")]
fn page_url(w: &mut PageUrlWorld, url: String) {
    w.url = url;
}

#[then("the URL is allowed")]
fn allowed(w: &mut PageUrlWorld) {
    assert_eq!(check_page_url(&w.url), Ok(()));
}

#[then("the URL is rejected")]
fn rejected(w: &mut PageUrlWorld) {
    assert!(check_page_url(&w.url).is_err(), "expected a rejection");
}

/// Runs only `catalog-page-url.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = PageUrlWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("catalog-page-url.feature"))
        .await;
    !writer.execution_has_failed()
}
