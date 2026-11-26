//! Steps for `tests/features/ad-feed.feature` (issue #102).
//! The real `AdService` is composed with the real adapters (`HttpAdSource`,
//! `JsonAdCache`, `HttpAdBeaconSink`). A local 127.0.0.1 server (or a refused
//! local port for "offline"), a temp dir and a fixed clock: never the internet.
use crate::feature;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AdCache, AdService, AdSource, AdSourceError, AdView, CachedAds, Clock};
use motogp_domain::{AdCreative, AdCreativeDraft, AdSlot};
use motogp_infrastructure::{DataDir, HttpAdBeaconSink, HttpAdConfig, HttpAdSource, JsonAdCache};
use std::net::TcpListener;
use std::time::Duration;
use tempfile::TempDir;

use crate::catalog_source::http_server::{Reply, TestServer};

/// 2026-06-15T12:00:00Z.
const NOW_MS: u64 = 1_781_524_800_000;
const DAY_MS: u64 = 86_400_000;
const FIVE_MIB: usize = 5 * 1024 * 1024;
const IMAGE_CAP: u64 = 200 * 1024;
const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

#[derive(Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        NOW_MS
    }
}

#[derive(Debug, Default, World)]
struct AdFeedWorld {
    tmp: Option<TempDir>,
    server: Option<TestServer>,
    ttl_days: Option<u64>,
    offline: bool,
    expect_too_large: bool,
    shown: Option<Result<Option<AdView>, String>>,
    fetched: Option<Result<(), AdSourceError>>,
    cache_before: Option<Vec<u8>>,
}

fn creative(id: &str, expires_at_ms: Option<u64>) -> AdCreative {
    AdCreative::from_draft(AdCreativeDraft {
        id: id.into(),
        slot: "about".into(),
        title: format!("Title of {id}"),
        body: "Body".into(),
        image_data_uri: Some(PNG.into()),
        click_url: "https://example.com/click".into(),
        sponsor_label: "Sponsor".into(),
        impression_url: None,
        expires_at_ms,
        ..Default::default()
    })
    .expect("valid creative")
}

fn feed_body() -> String {
    r#"{"version":1,"creatives":[{"id":"fresh","slot":"about","title":"Fresh","body":"B","image":"ads/fresh.png","click_url":"https://example.com/c","sponsor_label":"S"}]}"#.to_string()
}

impl AdFeedWorld {
    fn cache_file(&mut self) -> std::path::PathBuf {
        let dir = self
            .tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"));
        DataDir::new(dir.path()).ad_cache_file()
    }

    fn seed_cache(&mut self, creative: AdCreative, age_ms: u64) {
        let path = self.cache_file();
        JsonAdCache::new(path)
            .save(&CachedAds {
                fetched_at_ms: NOW_MS - age_ms,
                creatives: vec![creative],
            })
            .expect("seed cache");
    }

    fn config(&self) -> HttpAdConfig {
        let mut config = HttpAdConfig::github("9.9.9");
        config.timeout = Duration::from_secs(5);
        config.connect_timeout = Duration::from_secs(2);
        let base = match &self.server {
            Some(server) if !self.offline => {
                server.url.trim_end_matches("catalog.json").to_string()
            }
            _ => refused_base_url(),
        };
        config.url = format!("{base}ads.json");
        config.raw_base_url = base;
        config
    }

    fn source(&self) -> HttpAdSource<FixedClock> {
        HttpAdSource::new(self.config(), FixedClock).expect("client")
    }

    fn ask_service(&mut self) {
        let config = self.config();
        let ttl = Duration::from_secs(self.ttl_days.unwrap_or(0) * 86_400);
        let cache = JsonAdCache::new(self.cache_file());
        let beacon = HttpAdBeaconSink::new(&config).expect("beacon");
        let source = HttpAdSource::new(config, FixedClock).expect("client");
        let service = if self.ttl_days.is_some() {
            AdService::with_ttl(source, cache, beacon, FixedClock, ttl)
        } else {
            AdService::new(source, cache, beacon, FixedClock)
        };
        self.shown = Some(
            service
                .get_ad(AdSlot::About, true, None)
                .map_err(|e| e.to_string()),
        );
    }

    /// Serves a feed whose image is `image` (script repeats for the second ask).
    fn serve_feed_with_image(&mut self, image: Reply) {
        let feed = Reply::new(200)
            .header("Content-Type", "application/json")
            .body(feed_body());
        self.server = Some(TestServer::start(vec![
            feed.clone(),
            image.clone(),
            feed,
            image,
        ]));
        // A previous good feed is already cached, but stale, so the source is asked.
        self.seed_cache(creative("previous", None), 30 * DAY_MS);
    }
}

/// A base URL on a port nobody listens on.
fn refused_base_url() -> String {
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    format!("http://127.0.0.1:{port}/")
}

// ---------- Offline ----------

#[given(expr = "a creative was cached {int} day ago and the expiry is {int} days")]
fn cached_recently(w: &mut AdFeedWorld, days_ago: u64, ttl_days: u64) {
    w.ttl_days = Some(ttl_days);
    w.seed_cache(creative("cached", None), days_ago * DAY_MS);
}

#[given("the cached creative expired")]
fn cached_expired(w: &mut AdFeedWorld) {
    // Cached long ago, and its own expiry date is in the past.
    w.seed_cache(creative("cached", Some(NOW_MS - DAY_MS)), 10 * DAY_MS);
}

#[given("the network is unavailable")]
fn network_unavailable(w: &mut AdFeedWorld) {
    w.offline = true;
}

#[when("the UI asks for the ad")]
fn ui_asks(w: &mut AdFeedWorld) {
    w.ask_service();
}

#[then("the cached creative is returned")]
fn cached_returned(w: &mut AdFeedWorld) {
    let shown = w.shown.clone().expect("not asked");
    let view = shown.expect("no error").expect("a creative");
    assert_eq!(view.creative_id, "cached");
    assert_eq!(view.title, "Title of cached");
}

#[then("no creative is returned and no error is shown")]
fn nothing_returned(w: &mut AdFeedWorld) {
    assert_eq!(w.shown, Some(Ok(None)));
}

// ---------- Refused images ----------

#[given("the feed points to a 5 MiB image")]
fn five_mib_image(w: &mut AdFeedWorld) {
    w.expect_too_large = true;
    w.serve_feed_with_image(
        Reply::new(200)
            .header("Content-Type", "image/png")
            .body(vec![1u8; FIVE_MIB]),
    );
}

#[given(expr = "the feed points to an image served as {string}")]
fn image_served_as(w: &mut AdFeedWorld, content_type: String) {
    w.serve_feed_with_image(
        Reply::new(200)
            .header("Content-Type", &content_type)
            .body("<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
    );
}

#[when("the creative is fetched")]
fn creative_fetched(w: &mut AdFeedWorld) {
    let path = w.cache_file();
    w.cache_before = Some(std::fs::read(&path).expect("seeded cache"));
    w.fetched = Some(w.source().fetch().map(|_| ()));
    w.ask_service();
}

#[then("it is rejected and the previous cache is kept")]
fn rejected_and_cache_kept(w: &mut AdFeedWorld) {
    let error = w
        .fetched
        .clone()
        .expect("not fetched")
        .expect_err("the feed must be rejected");
    if w.expect_too_large {
        assert_eq!(error, AdSourceError::TooLarge { limit: IMAGE_CAP });
    } else {
        assert!(matches!(error, AdSourceError::Invalid(_)), "{error:?}");
    }
    let path = w.cache_file();
    assert_eq!(
        std::fs::read(path).expect("cache file"),
        w.cache_before.clone().expect("snapshot"),
        "the cache file must be byte-for-byte unchanged"
    );
    let view = w
        .shown
        .clone()
        .expect("not asked")
        .expect("no error")
        .expect("the previous creative");
    assert_eq!(view.creative_id, "previous");
}

/// Runs only `ad-feed.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = AdFeedWorld::cucumber()
        .fail_on_skipped()
        .run(feature("ad-feed.feature"))
        .await;
    !writer.execution_has_failed()
}
