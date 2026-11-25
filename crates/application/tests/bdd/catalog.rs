//! Steps for `tests/features/catalog.feature` (issue #72), with in-memory fakes
//! of the catalog ports. Traceability: docs/validation-workflow.md.
use crate::catalog_fakes::*;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    CatalogError, CatalogSnapshot, CatalogSourceError, RawEntry, SourceFetch,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Default, World)]
struct CatalogWorld {
    source: Option<FakeSource>,
    cache: FakeCache,
    clock: Option<FakeClock>,
    result: Option<Result<CatalogSnapshot, CatalogError>>,
    pair: Option<(CatalogSnapshot, CatalogSnapshot)>,
    /// Kept between steps: the backoff lives inside the service.
    svc: Option<Arc<TestService>>,
}

impl std::fmt::Debug for CatalogWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CatalogWorld")
    }
}

impl CatalogWorld {
    fn clock(&mut self) -> FakeClock {
        self.clock
            .get_or_insert_with(|| FakeClock::at(NOW_MS))
            .clone()
    }
    fn source(&self) -> FakeSource {
        self.source.clone().expect("no source configured")
    }
    fn service(&mut self) -> Arc<TestService> {
        let clock = self.clock();
        let (source, cache) = (self.source(), self.cache.clone());
        self.svc
            .get_or_insert_with(|| Arc::new(service(&source, &cache, &clock)))
            .clone()
    }
    fn snapshot(&self) -> &CatalogSnapshot {
        self.result
            .as_ref()
            .expect("catalog not requested")
            .as_ref()
            .expect("request failed")
    }
    fn error(&self) -> &CatalogError {
        self.result
            .as_ref()
            .expect("catalog not requested")
            .as_ref()
            .expect_err("expected a failure")
    }
    fn cache_aged(&mut self, hours: u64) {
        self.cache = FakeCache::holding(cached(NOW_MS - hours * HOUR_MS, &["old"]));
    }
}

#[given(expr = "the catalog was cached {int} hour ago and the TTL is {int} hours")]
fn cached_fresh(w: &mut CatalogWorld, hours: u64, _ttl: u64) {
    // The TTL is the default 6 hours; the step documents it.
    w.cache_aged(hours);
    w.source = Some(FakeSource::serving(vec![raw("new")]));
}

#[given(expr = "the catalog was cached {int} hours ago")]
fn cached_stale(w: &mut CatalogWorld, hours: u64) {
    w.cache_aged(hours);
    w.source = Some(FakeSource::serving(vec![raw("new")]));
}

#[given("the cache is stale and the source is unreachable")]
fn stale_offline(w: &mut CatalogWorld) {
    w.cache_aged(7);
    w.source = Some(FakeSource::failing(CatalogSourceError::Network(
        "down".into(),
    )));
}

#[given("there is no cache and the source is unreachable")]
fn no_cache_offline(w: &mut CatalogWorld) {
    w.source = Some(FakeSource::failing(CatalogSourceError::Network(
        "down".into(),
    )));
}

#[given(expr = "the source returns 3 entries and one has a {string} page URL")]
fn three_one_bad(w: &mut CatalogWorld, scheme: String) {
    let mut bad = draft("bad");
    bad.page_url = format!("{scheme}C:/Windows");
    w.source = Some(FakeSource::serving(vec![
        raw("one"),
        RawEntry::Draft(bad),
        raw("three"),
    ]));
}

#[given(expr = "the source returns two entries with the id {string}")]
fn two_with_same_id(w: &mut CatalogWorld, id: String) {
    let mut second = draft(&id);
    second.title = "Second".into();
    w.source = Some(FakeSource::serving(vec![
        RawEntry::Draft(draft(&id)),
        RawEntry::Draft(second),
    ]));
}

#[given("the cache is stale")]
fn stale(w: &mut CatalogWorld) {
    w.cache_aged(7);
    w.source = Some(FakeSource::serving(vec![raw("new")]));
}

#[given(expr = "the source answered {string} with Retry-After {int}")]
fn rate_limited(w: &mut CatalogWorld, _what: String, secs: u64) {
    w.source = Some(FakeSource::failing(CatalogSourceError::RateLimited {
        retry_after_secs: Some(secs),
    }));
    let _ = w.service().refresh();
    assert_eq!(w.source().calls(), 1);
}

#[given(expr = "the source returns schemaVersion {int}")]
fn schema_version(w: &mut CatalogWorld, version: u32) {
    w.source = Some(FakeSource::answering(vec![Ok(fetched(
        version,
        vec![raw("a")],
        None,
    ))]));
}

#[given(expr = "the cache is stale and the source answers {string}")]
fn stale_not_modified(w: &mut CatalogWorld, _what: String) {
    w.cache_aged(7);
    w.source = Some(FakeSource::answering(vec![Ok(SourceFetch::NotModified)]));
}

#[when("the catalog is requested")]
fn request(w: &mut CatalogWorld) {
    w.result = Some(w.service().get());
}

#[when("the catalog is requested with no cache")]
fn request_no_cache(w: &mut CatalogWorld) {
    w.cache = FakeCache::default();
    w.svc = None;
    w.result = Some(w.service().get());
}

#[when("two requests arrive at the same time")]
fn two_requests(w: &mut CatalogWorld) {
    let gate = w.source().gated();
    let svc = w.service();
    let first = {
        let svc = svc.clone();
        thread::spawn(move || svc.get())
    };
    gate.wait_entered();
    let second = {
        let svc = svc.clone();
        thread::spawn(move || svc.get())
    };
    thread::sleep(Duration::from_millis(100));
    gate.open();
    let a = first.join().unwrap().expect("first request");
    let b = second.join().unwrap().expect("second request");
    w.pair = Some((a, b));
}

#[when(expr = "the catalog is refreshed again {int} seconds later")]
fn refresh_later(w: &mut CatalogWorld, secs: u64) {
    w.clock().advance(secs * 1000);
    w.result = Some(w.service().refresh());
}

#[then("the cached snapshot is returned without calling the source")]
fn served_from_cache(w: &mut CatalogWorld) {
    assert_eq!(w.source().calls(), 0);
    let snap = w.snapshot();
    assert!(!snap.stale);
    assert_eq!(snap.entries[0].id, "old");
}

#[then("the source is called once and the new snapshot is cached")]
fn fetched_and_cached(w: &mut CatalogWorld) {
    assert_eq!(w.source().calls(), 1);
    assert_eq!(w.snapshot().entries[0].id, "new");
    let stored = w.cache.stored().expect("nothing cached");
    assert_eq!(stored.fetched_at_ms, NOW_MS);
    assert_eq!(stored.entries[0].id, "new");
}

#[then("the cached snapshot is returned with the stale flag")]
fn stale_flag(w: &mut CatalogWorld) {
    let snap = w.snapshot();
    assert!(snap.stale);
    assert_eq!(snap.entries[0].id, "old");
}

#[then("the request fails as unavailable")]
fn unavailable(w: &mut CatalogWorld) {
    assert!(matches!(w.error(), CatalogError::Unavailable { .. }));
}

#[then(expr = "{int} entries are returned and {int} entry is reported as skipped")]
fn counts(w: &mut CatalogWorld, entries: usize, skipped: usize) {
    let snap = w.snapshot();
    assert_eq!(snap.entries.len(), entries);
    assert_eq!(snap.skipped, skipped);
    assert_eq!(snap.warnings.len(), skipped);
}

#[then(expr = "only the first one is kept and {int} entry is reported as skipped")]
fn first_kept(w: &mut CatalogWorld, skipped: usize) {
    let snap = w.snapshot();
    assert_eq!(snap.entries.len(), 1);
    assert_eq!(snap.entries[0].title, "Title of rebalanced-ai");
    assert_eq!(snap.skipped, skipped);
}

#[then("the source is called only once")]
fn called_once(w: &mut CatalogWorld) {
    assert_eq!(w.source().calls(), 1);
    let (a, b) = w.pair.as_ref().expect("no concurrent requests");
    assert_eq!(a, b);
}

#[then("the source is not called")]
fn not_called(w: &mut CatalogWorld) {
    assert_eq!(w.source().calls(), 1, "only the first answer was fetched");
    assert!(matches!(w.error(), CatalogError::Unavailable { .. }));
}

#[then("the request fails as an unsupported schema")]
fn unsupported(w: &mut CatalogWorld) {
    assert_eq!(w.error(), &CatalogError::UnsupportedSchema { version: 2 });
}

#[then("the cached entries are returned as fresh")]
fn fresh(w: &mut CatalogWorld) {
    let snap = w.snapshot();
    assert!(!snap.stale);
    assert_eq!(snap.entries[0].id, "old");
    assert_eq!(snap.fetched_at_ms, NOW_MS);
    assert_eq!(w.cache.stored().unwrap().fetched_at_ms, NOW_MS);
}

/// Runs only `catalog.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CatalogWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("catalog.feature"))
        .await;
    !writer.execution_has_failed()
}
