//! Steps for `tests/features/catalog-source.feature` (issue #72).
//! A local 127.0.0.1 server and temporary directories only: never the internet.
use crate::feature;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    CachedCatalog, CatalogCache, CatalogSource, CatalogSourceError, RawEntry, SourceFetch,
};
use motogp_domain::{CatalogEntry, CatalogEntryDraft};
use motogp_infrastructure::{DataDir, HttpCatalogConfig, HttpCatalogSource, JsonCatalogCache};
use std::time::Duration;
use tempfile::TempDir;

#[path = "../support/http_server.rs"]
mod http_server;
use http_server::{Reply, TestServer};

const USER_AGENT: &str = "motogp-launcher/9.9.9 (+https://github.com/IcaroSilvaFK/motogp-launcher)";
const SMALL_CAP: u64 = 200;

#[derive(Debug, Default, World)]
struct CatalogSourceWorld {
    server: Option<TestServer>,
    max_bytes: Option<u64>,
    outcome: Option<Result<SourceFetch, CatalogSourceError>>,
    tmp: Option<TempDir>,
    cache: Option<JsonCatalogCache>,
    saved: Option<CachedCatalog>,
    loaded: Option<Option<CachedCatalog>>,
}

fn entry_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","title":"T","author":"A","game":"MotoGP 26","category":"Gameplay","description":"D","version":"1.0","updatedAt":"2026-01-02","pageUrl":"https://example.com/{id}"}}"#
    )
}

fn catalog_body(count: usize) -> String {
    let entries: Vec<String> = (1..=count)
        .map(|i| entry_json(&format!("mod-{i}")))
        .collect();
    format!(r#"{{"schemaVersion":1,"entries":[{}]}}"#, entries.join(","))
}

impl CatalogSourceWorld {
    fn start(&mut self, reply: Reply) {
        self.server = Some(TestServer::start(vec![reply]));
    }

    fn server(&self) -> &TestServer {
        self.server.as_ref().expect("server not started")
    }

    fn fetch(&mut self, etag: Option<&str>) {
        let mut config = HttpCatalogConfig::github("9.9.9");
        config.url = self.server().url.clone();
        config.timeout = Duration::from_secs(5);
        if let Some(max) = self.max_bytes {
            config.max_bytes = max;
        }
        let source = HttpCatalogSource::new(config).expect("client");
        self.outcome = Some(source.fetch(etag));
    }
}

// ---------- Fetch and parse ----------

#[given(expr = "a local server serving a valid catalog with {int} entries and ETag {string}")]
fn serving_catalog(w: &mut CatalogSourceWorld, count: usize, etag: String) {
    w.start(
        Reply::new(200)
            .header("ETag", &etag)
            .body(catalog_body(count)),
    );
}

#[when("the source fetches it")]
fn source_fetches(w: &mut CatalogSourceWorld) {
    w.fetch(None);
}

#[then(expr = "{int} drafts are returned with the ETag {string}")]
fn drafts_returned(w: &mut CatalogSourceWorld, count: usize, etag: String) {
    let Some(Ok(SourceFetch::Fetched { catalog, etag: got })) = &w.outcome else {
        panic!("expected Fetched, got {:?}", w.outcome);
    };
    assert_eq!(catalog.entries.len(), count);
    assert!(catalog
        .entries
        .iter()
        .all(|e| matches!(e, RawEntry::Draft(CatalogEntryDraft { .. }))));
    assert_eq!(got.as_deref(), Some(etag.as_str()));
}

// ---------- Conditional request ----------

#[given(expr = "the server answers 304 when If-None-Match is {string}")]
fn answers_304(w: &mut CatalogSourceWorld, _etag: String) {
    w.start(Reply::new(304));
}

#[when(expr = "the source fetches with the ETag {string}")]
fn fetch_with_etag(w: &mut CatalogSourceWorld, etag: String) {
    w.fetch(Some(&etag));
}

#[then(expr = "the result is {string}")]
fn result_is(w: &mut CatalogSourceWorld, what: String) {
    assert_eq!(what, "not modified");
    assert_eq!(w.outcome, Some(Ok(SourceFetch::NotModified)));
}

#[then(expr = "the server received If-None-Match {string}")]
fn server_received_etag(w: &mut CatalogSourceWorld, etag: String) {
    let requests = w.server().requests();
    assert_eq!(requests[0].header("if-none-match"), Some(etag.as_str()));
}

// ---------- Size cap ----------

#[given("the server sends a body larger than the cap")]
fn body_over_cap(w: &mut CatalogSourceWorld) {
    w.max_bytes = Some(SMALL_CAP);
    let body = catalog_body(5);
    assert!(body.len() as u64 > SMALL_CAP);
    w.start(Reply::new(200).body(body));
}

#[then("it fails as too large")]
fn fails_too_large(w: &mut CatalogSourceWorld) {
    assert_eq!(
        w.outcome,
        Some(Err(CatalogSourceError::TooLarge { limit: SMALL_CAP }))
    );
}

// ---------- Rate limit ----------

#[given(expr = "the server answers 429 with Retry-After {int}")]
fn answers_429(w: &mut CatalogSourceWorld, secs: u64) {
    w.start(Reply::new(429).header("Retry-After", &secs.to_string()));
}

#[then(expr = "it fails as rate limited for {int} seconds")]
fn fails_rate_limited(w: &mut CatalogSourceWorld, secs: u64) {
    assert_eq!(
        w.outcome,
        Some(Err(CatalogSourceError::RateLimited {
            retry_after_secs: Some(secs)
        }))
    );
}

// ---------- User-Agent ----------

#[then("the request carries the launcher User-Agent")]
fn carries_user_agent(w: &mut CatalogSourceWorld) {
    let requests = w.server().requests();
    assert_eq!(requests[0].header("user-agent"), Some(USER_AGENT));
}

// ---------- Cache roundtrip ----------

#[given(expr = "a data folder {string}")]
fn data_folder(w: &mut CatalogSourceWorld, name: String) {
    // Non-ASCII on purpose: Windows user folders often contain accents (e.g. "Usuário Padrão").
    let root = w
        .tmp
        .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
        .path()
        .join(name)
        .join("data");
    w.cache = Some(JsonCatalogCache::new(
        DataDir::new(root).catalog_cache_file(),
    ));
}

#[when("a snapshot is saved and loaded")]
fn save_and_load(w: &mut CatalogSourceWorld) {
    let entry = CatalogEntry::from_draft(CatalogEntryDraft {
        id: "rebalanced-ai".into(),
        title: "Rebalanced AI".into(),
        author: "Someone".into(),
        game: "MotoGP 26".into(),
        category: "Gameplay".into(),
        description: "Better AI".into(),
        version: "1.0".into(),
        updated_at: "2026-01-02".into(),
        page_url: "https://example.com/rebalanced-ai".into(),
        thumbnail_url: None,
        download: None,
    })
    .unwrap();
    let snapshot = CachedCatalog {
        fetched_at_ms: 1_700_000_000_000,
        etag: Some("\"v1\"".into()),
        entries: vec![entry],
        skipped: 0,
    };
    let cache = w.cache.as_ref().expect("cache not set");
    cache.save(&snapshot).expect("save");
    w.loaded = Some(cache.load().expect("load"));
    w.saved = Some(snapshot);
}

#[then("the same entries and timestamp come back")]
fn same_snapshot(w: &mut CatalogSourceWorld) {
    assert_eq!(w.loaded, Some(w.saved.clone()));
}

/// Runs only `catalog-source.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CatalogSourceWorld::cucumber()
        .fail_on_skipped()
        .run(feature("catalog-source.feature"))
        .await;
    !writer.execution_has_failed()
}
