//! Use-case tests of the catalog service with in-memory fakes (issue #72).
#[path = "support/catalog_fakes.rs"]
mod fakes;

use fakes::*;
use motogp_application::{
    validate_catalog, validate_catalog_strict, CatalogCacheError, CatalogError, CatalogService,
    CatalogSourceError, RawCatalog, RawEntry, SourceFetch, DEFAULT_CATALOG_TTL,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn offline() -> CatalogSourceError {
    CatalogSourceError::Network("unreachable".into())
}

fn ids(entries: &[motogp_domain::CatalogEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.id.as_str()).collect()
}

#[test]
fn default_ttl_is_six_hours() {
    assert_eq!(DEFAULT_CATALOG_TTL, Duration::from_secs(6 * 3600));
}

#[test]
fn service_is_send_and_sync() {
    fn check<T: Send + Sync>() {}
    check::<TestService>();
}

#[test]
fn fresh_cache_is_served_without_calling_the_source() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::holding(cached(NOW_MS - HOUR_MS, &["old"]));

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(source.calls(), 0);
    assert_eq!(ids(&snap.entries), ["old"]);
    assert!(!snap.stale);
    assert_eq!(snap.fetched_at_ms, NOW_MS - HOUR_MS);
}

#[test]
fn stale_cache_is_refreshed_and_saved_sending_the_etag() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(source.calls(), 1);
    assert_eq!(source.etags_seen(), [Some("etag-cached".to_string())]);
    assert_eq!(ids(&snap.entries), ["new"]);
    assert!(!snap.stale);
    let stored = cache.stored().unwrap();
    assert_eq!(stored.fetched_at_ms, NOW_MS);
    assert_eq!(stored.etag.as_deref(), Some("etag-1"));
    assert_eq!(ids(&stored.entries), ["new"]);
}

#[test]
fn empty_cache_fetches_without_an_etag() {
    let source = FakeSource::serving(vec![raw("a")]);
    let cache = FakeCache::default();

    service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(source.etags_seen(), [None]);
}

#[test]
fn custom_ttl_is_honored() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::holding(cached(NOW_MS - HOUR_MS, &["old"]));
    let svc = CatalogService::with_ttl(
        source.clone(),
        cache,
        FakeClock::at(NOW_MS),
        Duration::from_secs(1800),
    );

    svc.get().unwrap();

    assert_eq!(source.calls(), 1);
}

#[test]
fn refresh_ignores_the_ttl() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::holding(cached(NOW_MS - 1000, &["old"]));

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .refresh()
        .unwrap();

    assert_eq!(source.calls(), 1);
    assert_eq!(ids(&snap.entries), ["new"]);
}

#[test]
fn offline_with_a_stale_cache_returns_it_flagged_stale() {
    let source = FakeSource::failing(offline());
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert!(snap.stale);
    assert_eq!(ids(&snap.entries), ["old"]);
    assert_eq!(snap.fetched_at_ms, NOW_MS - 7 * HOUR_MS);
    assert_eq!(cache.saves(), 0);
}

#[test]
fn offline_without_a_cache_is_unavailable() {
    let source = FakeSource::failing(offline());

    let error = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS))
        .get()
        .unwrap_err();

    match error {
        CatalogError::Unavailable { reason } => assert!(reason.contains("unreachable")),
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn unreadable_cache_counts_as_no_cache() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::holding(cached(NOW_MS, &["old"]));
    cache.fail_loads();

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(source.calls(), 1);
    assert_eq!(ids(&snap.entries), ["new"]);
}

#[test]
fn a_cache_write_error_does_not_fail_a_good_fetch() {
    let source = FakeSource::serving(vec![raw("new")]);
    let cache = FakeCache::default();
    cache.fail_saves();

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(ids(&snap.entries), ["new"]);
    assert!(cache.stored().is_none());
}

#[test]
fn malformed_entries_are_skipped_with_warnings() {
    let mut bad = draft("evil");
    bad.page_url = "file:///C:/Windows".into();
    let source = FakeSource::serving(vec![
        raw("a"),
        RawEntry::Draft(bad),
        raw("b"),
        RawEntry::Unparseable {
            reason: "entry 3 is not an object".into(),
        },
    ]);
    let cache = FakeCache::default();

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(ids(&snap.entries), ["a", "b"]);
    assert_eq!(snap.skipped, 2);
    assert_eq!(snap.warnings.len(), 2);
    assert_eq!(snap.warnings[0].index, 1);
    assert_eq!(snap.warnings[0].id.as_deref(), Some("evil"));
    assert!(snap.warnings[0].reason.contains("pageUrl"));
    assert_eq!(snap.warnings[1].index, 3);
    assert_eq!(snap.warnings[1].id, None);
    assert_eq!(snap.warnings[1].reason, "entry 3 is not an object");
    assert_eq!(cache.stored().unwrap().skipped, 2);
}

#[test]
fn an_entry_with_a_blank_id_has_no_id_in_its_warning() {
    let mut bad = draft("x");
    bad.id = "  ".into();
    let source = FakeSource::serving(vec![RawEntry::Draft(bad)]);

    let snap = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(snap.warnings[0].id, None);
}

#[test]
fn duplicate_ids_keep_the_first() {
    let mut second = draft("rebalanced-ai");
    second.title = "Second".into();
    let source = FakeSource::serving(vec![raw("rebalanced-ai"), RawEntry::Draft(second)]);

    let snap = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert_eq!(snap.entries.len(), 1);
    assert_eq!(snap.entries[0].title, "Title of rebalanced-ai");
    assert_eq!(snap.skipped, 1);
    assert_eq!(snap.warnings[0].reason, "duplicate id");
}

#[test]
fn a_newer_schema_without_cache_is_unsupported() {
    let source = FakeSource::answering(vec![Ok(fetched(2, vec![raw("a")], None))]);

    let error = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS))
        .get()
        .unwrap_err();

    assert_eq!(error, CatalogError::UnsupportedSchema { version: 2 });
}

#[test]
fn a_newer_schema_with_a_cache_falls_back_to_it() {
    let source = FakeSource::answering(vec![Ok(fetched(2, vec![raw("a")], None))]);
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));

    let snap = service(&source, &cache, &FakeClock::at(NOW_MS))
        .get()
        .unwrap();

    assert!(snap.stale);
    assert_eq!(ids(&snap.entries), ["old"]);
}

#[test]
fn the_unsupported_schema_error_survives_the_backoff() {
    let source = FakeSource::answering(vec![Ok(fetched(2, vec![], None))]);
    let svc = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS));
    svc.get().unwrap_err();

    let error = svc.get().unwrap_err();

    assert_eq!(error, CatalogError::UnsupportedSchema { version: 2 });
    assert_eq!(source.calls(), 1);
}

#[test]
fn not_modified_renews_the_cache_timestamp() {
    let source = FakeSource::answering(vec![Ok(SourceFetch::NotModified)]);
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));
    let clock = FakeClock::at(NOW_MS);

    let snap = service(&source, &cache, &clock).get().unwrap();

    assert!(!snap.stale);
    assert_eq!(ids(&snap.entries), ["old"]);
    assert_eq!(snap.fetched_at_ms, NOW_MS);
    assert_eq!(cache.stored().unwrap().fetched_at_ms, NOW_MS);
    assert_eq!(cache.stored().unwrap().etag.as_deref(), Some("etag-cached"));
    // Fresh again: the next call does not hit the source.
    service(&source, &cache, &clock).get().unwrap();
    assert_eq!(source.calls(), 1);
}

#[test]
fn not_modified_without_a_cache_is_a_failure() {
    let source = FakeSource::answering(vec![Ok(SourceFetch::NotModified)]);

    let error = service(&source, &FakeCache::default(), &FakeClock::at(NOW_MS))
        .get()
        .unwrap_err();

    assert!(matches!(error, CatalogError::Unavailable { .. }));
}

#[test]
fn two_concurrent_stale_requests_fetch_once() {
    let source = FakeSource::serving(vec![raw("new")]);
    let gate = source.gated();
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));
    let svc = Arc::new(service(&source, &cache, &FakeClock::at(NOW_MS)));

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

    let a = first.join().unwrap().unwrap();
    let b = second.join().unwrap().unwrap();
    assert_eq!(source.calls(), 1);
    assert_eq!(a, b);
    assert_eq!(ids(&a.entries), ["new"]);
}

#[test]
fn concurrent_explicit_refreshes_share_one_failure() {
    let source = FakeSource::failing(offline());
    let gate = source.gated();
    let svc = Arc::new(service(
        &source,
        &FakeCache::default(),
        &FakeClock::at(NOW_MS),
    ));

    let first = {
        let svc = svc.clone();
        thread::spawn(move || svc.refresh())
    };
    gate.wait_entered();
    let second = {
        let svc = svc.clone();
        thread::spawn(move || svc.refresh())
    };
    thread::sleep(Duration::from_millis(100));
    gate.open();

    assert!(first.join().unwrap().is_err());
    assert!(second.join().unwrap().is_err());
    assert_eq!(source.calls(), 1);
}

fn rate_limited(secs: Option<u64>) -> CatalogSourceError {
    CatalogSourceError::RateLimited {
        retry_after_secs: secs,
    }
}

#[test]
fn retry_after_blocks_the_source_until_it_elapses() {
    let source = FakeSource::failing(rate_limited(Some(60)));
    let cache = FakeCache::default();
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &cache, &clock);
    svc.refresh().unwrap_err();
    assert_eq!(source.calls(), 1);

    clock.advance(30_000);
    svc.refresh().unwrap_err();
    assert_eq!(source.calls(), 1);

    clock.advance(30_000);
    svc.refresh().unwrap_err();
    assert_eq!(source.calls(), 2);
}

#[test]
fn backoff_also_protects_get_and_serves_the_stale_cache() {
    let source = FakeSource::failing(offline());
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));
    let svc = service(&source, &cache, &FakeClock::at(NOW_MS));
    svc.get().unwrap();

    let snap = svc.get().unwrap();

    assert!(snap.stale);
    assert_eq!(source.calls(), 1);
}

#[test]
fn backoff_doubles_from_30_seconds_up_to_one_hour() {
    let source = FakeSource::failing(offline());
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &FakeCache::default(), &clock);
    let mut expected_calls = 0;
    for secs in [30u64, 60, 120, 240, 480, 960, 1920, 3600, 3600] {
        svc.refresh().unwrap_err();
        expected_calls += 1;
        assert_eq!(source.calls(), expected_calls);
        clock.advance(secs * 1000 - 1);
        svc.refresh().unwrap_err();
        assert_eq!(source.calls(), expected_calls, "still waiting {secs}s");
        clock.advance(1);
    }
}

#[test]
fn many_failures_do_not_overflow_the_backoff() {
    let source = FakeSource::failing(offline());
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &FakeCache::default(), &clock);
    for _ in 0..40 {
        svc.refresh().unwrap_err();
        clock.advance(BACKOFF_CAP_MS);
    }
    assert_eq!(source.calls(), 40);
}

const BACKOFF_CAP_MS: u64 = 3_600_000;

#[test]
fn retry_after_is_capped_at_one_hour() {
    let source = FakeSource::failing(rate_limited(Some(u64::MAX)));
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &FakeCache::default(), &clock);
    svc.refresh().unwrap_err();

    clock.advance(BACKOFF_CAP_MS);
    svc.refresh().unwrap_err();

    assert_eq!(source.calls(), 2);
}

#[test]
fn rate_limited_without_retry_after_uses_the_exponential_backoff() {
    let source = FakeSource::failing(rate_limited(None));
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &FakeCache::default(), &clock);
    svc.refresh().unwrap_err();

    clock.advance(29_999);
    svc.refresh().unwrap_err();
    assert_eq!(source.calls(), 1);
    clock.advance(1);
    svc.refresh().unwrap_err();
    assert_eq!(source.calls(), 2);
}

#[test]
fn a_success_resets_the_backoff() {
    let source = FakeSource::failing(offline());
    source.then(Ok(fetched(1, vec![raw("a")], None)));
    source.then(Err(offline()));
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &FakeCache::default(), &clock);
    svc.refresh().unwrap_err(); // wait 30 s
    clock.advance(30_000);
    svc.refresh().unwrap(); // success, resets
    assert!(svc.refresh().unwrap().stale); // fails again: wait is 30 s, not 60 s
    assert_eq!(source.calls(), 3);

    clock.advance(30_000);
    assert!(svc.refresh().unwrap().stale);

    assert_eq!(source.calls(), 4);
}

#[test]
fn not_modified_also_resets_the_backoff() {
    let source = FakeSource::failing(offline());
    source.then(Ok(SourceFetch::NotModified));
    source.then(Err(offline()));
    let cache = FakeCache::holding(cached(NOW_MS - 7 * HOUR_MS, &["old"]));
    let clock = FakeClock::at(NOW_MS);
    let svc = service(&source, &cache, &clock);
    svc.get().unwrap(); // failure
    clock.advance(30_000);
    svc.refresh().unwrap(); // not modified
    svc.refresh().unwrap(); // fails, wait 30 s
    clock.advance(30_000);

    svc.refresh().unwrap();

    assert_eq!(source.calls(), 4);
}

fn raw_catalog(schema_version: u32, entries: Vec<RawEntry>) -> RawCatalog {
    RawCatalog {
        schema_version,
        entries,
    }
}

#[test]
fn validate_catalog_reports_the_unsupported_schema() {
    let error = validate_catalog(raw_catalog(9, vec![])).unwrap_err();
    assert_eq!(error, CatalogError::UnsupportedSchema { version: 9 });
}

#[test]
fn strict_validation_accepts_a_clean_catalog() {
    let entries = validate_catalog_strict(raw_catalog(1, vec![raw("a"), raw("b")])).unwrap();
    assert_eq!(ids(&entries), ["a", "b"]);
    assert!(validate_catalog_strict(raw_catalog(1, vec![]))
        .unwrap()
        .is_empty());
}

#[test]
fn strict_validation_rejects_any_other_schema_version() {
    for version in [0, 2] {
        let problems = validate_catalog_strict(raw_catalog(version, vec![])).unwrap_err();
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("schemaVersion"));
    }
}

#[test]
fn strict_validation_lists_every_skipped_entry() {
    let mut bad = draft("bad");
    bad.title = " ".into();
    let problems = validate_catalog_strict(raw_catalog(
        1,
        vec![
            raw("a"),
            RawEntry::Draft(bad),
            raw("a"),
            RawEntry::Unparseable {
                reason: "not an object".into(),
            },
        ],
    ))
    .unwrap_err();
    assert_eq!(problems.len(), 3);
    assert!(problems[0].contains("entry 1") && problems[0].contains("bad"));
    assert!(problems[1].contains("duplicate id"));
    assert!(problems[2].contains("(?)") && problems[2].contains("not an object"));
}

#[test]
fn errors_have_readable_messages() {
    let source_errors = [
        CatalogSourceError::Network("dns".into()),
        CatalogSourceError::Http { status: 500 },
        rate_limited(Some(60)),
        rate_limited(None),
        CatalogSourceError::TooLarge { limit: 2048 },
        CatalogSourceError::Invalid("not json".into()),
    ];
    let texts: Vec<String> = source_errors.iter().map(|e| e.to_string()).collect();
    assert!(texts[0].contains("dns"));
    assert!(texts[1].contains("500"));
    assert!(texts[2].contains("60"));
    assert_eq!(texts[3], "rate limited");
    assert!(texts[4].contains("2048"));
    assert!(texts[5].contains("not json"));
    assert!(CatalogCacheError("x".into()).to_string().contains('x'));
    assert!(CatalogError::Unavailable { reason: "r".into() }
        .to_string()
        .contains('r'));
    assert!(CatalogError::UnsupportedSchema { version: 3 }
        .to_string()
        .contains('3'));
    let _: &dyn std::error::Error = &source_errors[0];
    let _: &dyn std::error::Error = &CatalogCacheError("x".into());
    let _: &dyn std::error::Error = &CatalogError::UnsupportedSchema { version: 3 };
}
