//! In-memory fakes of the catalog ports (issue #72). No network, no disk, no real clock.
#![allow(dead_code)]
use motogp_application::{
    CachedCatalog, CatalogCache, CatalogCacheError, CatalogService, CatalogSource,
    CatalogSourceError, Clock, RawCatalog, RawEntry, SourceFetch,
};
use motogp_domain::{CatalogEntry, CatalogEntryDraft};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

pub const HOUR_MS: u64 = 60 * 60 * 1000;
/// Arbitrary "now" far from zero so "7 hours ago" never underflows.
pub const NOW_MS: u64 = 1_800_000_000_000;

#[derive(Clone)]
pub struct FakeClock(pub Arc<AtomicU64>);

impl FakeClock {
    pub fn at(ms: u64) -> Self {
        Self(Arc::new(AtomicU64::new(ms)))
    }
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_millis(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Lets a test hold the source inside `fetch` until it opens the gate.
#[derive(Default)]
pub struct Gate {
    state: Mutex<(bool, bool)>, // (entered, open)
    changed: Condvar,
}

impl Gate {
    pub fn wait_entered(&self) {
        let mut state = self.state.lock().unwrap();
        while !state.0 {
            state = self.changed.wait(state).unwrap();
        }
    }
    pub fn open(&self) {
        self.state.lock().unwrap().1 = true;
        self.changed.notify_all();
    }
    fn pass(&self) {
        let mut state = self.state.lock().unwrap();
        state.0 = true;
        self.changed.notify_all();
        while !state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }
}

#[derive(Default)]
struct SourceInner {
    script: Mutex<VecDeque<Result<SourceFetch, CatalogSourceError>>>,
    calls: AtomicUsize,
    etags: Mutex<Vec<Option<String>>>,
    gate: Mutex<Option<Arc<Gate>>>,
}

/// Replays a script of answers; the last one repeats forever.
#[derive(Clone, Default)]
pub struct FakeSource(Arc<SourceInner>);

impl FakeSource {
    pub fn answering(answers: Vec<Result<SourceFetch, CatalogSourceError>>) -> Self {
        let source = Self::default();
        *source.0.script.lock().unwrap() = answers.into();
        source
    }
    pub fn serving(entries: Vec<RawEntry>) -> Self {
        Self::answering(vec![Ok(fetched(1, entries, Some("etag-1")))])
    }
    pub fn failing(error: CatalogSourceError) -> Self {
        Self::answering(vec![Err(error)])
    }
    pub fn then(&self, answer: Result<SourceFetch, CatalogSourceError>) {
        self.0.script.lock().unwrap().push_back(answer);
    }
    pub fn calls(&self) -> usize {
        self.0.calls.load(Ordering::SeqCst)
    }
    pub fn etags_seen(&self) -> Vec<Option<String>> {
        self.0.etags.lock().unwrap().clone()
    }
    pub fn gated(&self) -> Arc<Gate> {
        let gate = Arc::new(Gate::default());
        *self.0.gate.lock().unwrap() = Some(gate.clone());
        gate
    }
}

impl CatalogSource for FakeSource {
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.etags.lock().unwrap().push(etag.map(String::from));
        let gate = self.0.gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.pass();
        }
        let mut script = self.0.script.lock().unwrap();
        if script.len() > 1 {
            script.pop_front().unwrap()
        } else {
            script.front().expect("empty script").clone()
        }
    }
}

#[derive(Default)]
struct CacheInner {
    stored: Mutex<Option<CachedCatalog>>,
    load_fails: Mutex<bool>,
    save_fails: Mutex<bool>,
    saves: AtomicUsize,
}

#[derive(Clone, Default)]
pub struct FakeCache(Arc<CacheInner>);

impl FakeCache {
    pub fn holding(cached: CachedCatalog) -> Self {
        let cache = Self::default();
        *cache.0.stored.lock().unwrap() = Some(cached);
        cache
    }
    pub fn stored(&self) -> Option<CachedCatalog> {
        self.0.stored.lock().unwrap().clone()
    }
    pub fn saves(&self) -> usize {
        self.0.saves.load(Ordering::SeqCst)
    }
    pub fn fail_loads(&self) {
        *self.0.load_fails.lock().unwrap() = true;
    }
    pub fn fail_saves(&self) {
        *self.0.save_fails.lock().unwrap() = true;
    }
}

impl CatalogCache for FakeCache {
    fn load(&self) -> Result<Option<CachedCatalog>, CatalogCacheError> {
        if *self.0.load_fails.lock().unwrap() {
            return Err(CatalogCacheError("corrupt".into()));
        }
        Ok(self.0.stored.lock().unwrap().clone())
    }
    fn save(&self, catalog: &CachedCatalog) -> Result<(), CatalogCacheError> {
        if *self.0.save_fails.lock().unwrap() {
            return Err(CatalogCacheError("disk full".into()));
        }
        self.0.saves.fetch_add(1, Ordering::SeqCst);
        *self.0.stored.lock().unwrap() = Some(catalog.clone());
        Ok(())
    }
}

pub type TestService = CatalogService<FakeSource, FakeCache, FakeClock>;

pub fn service(source: &FakeSource, cache: &FakeCache, clock: &FakeClock) -> TestService {
    CatalogService::new(source.clone(), cache.clone(), clock.clone())
}

pub fn draft(id: &str) -> CatalogEntryDraft {
    CatalogEntryDraft {
        id: id.into(),
        title: format!("Title of {id}"),
        author: "Author".into(),
        game: "MotoGP 26".into(),
        category: "Gameplay".into(),
        description: "Description.".into(),
        version: "1.0".into(),
        updated_at: "2026-11-20".into(),
        page_url: format!("https://example.com/{id}"),
        thumbnail_url: None,
        download: None,
    }
}

pub fn raw(id: &str) -> RawEntry {
    RawEntry::Draft(draft(id))
}

pub fn fetched(schema_version: u32, entries: Vec<RawEntry>, etag: Option<&str>) -> SourceFetch {
    SourceFetch::Fetched {
        catalog: RawCatalog {
            schema_version,
            entries,
        },
        etag: etag.map(String::from),
    }
}

pub fn entry(id: &str) -> CatalogEntry {
    CatalogEntry::from_draft(draft(id)).unwrap()
}

pub fn cached(fetched_at_ms: u64, ids: &[&str]) -> CachedCatalog {
    CachedCatalog {
        fetched_at_ms,
        etag: Some("etag-cached".into()),
        entries: ids.iter().map(|id| entry(id)).collect(),
        skipped: 0,
    }
}
