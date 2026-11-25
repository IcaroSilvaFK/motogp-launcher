use crate::{
    CachedCatalog, CatalogCache, CatalogError, CatalogSource, CatalogSourceError, Clock,
    RawCatalog, RawEntry, SourceFetch, WebPageOpener,
};
use motogp_domain::{check_page_url, CatalogEntry, SUPPORTED_CATALOG_SCHEMA};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long a fetched catalog is served without asking the source again.
pub const DEFAULT_CATALOG_TTL: Duration = Duration::from_secs(6 * 60 * 60);

/// First wait after a failed fetch; it doubles with every consecutive failure.
const BACKOFF_BASE_MS: u64 = 30_000;
/// No wait, computed or announced by the server, exceeds this.
const BACKOFF_MAX_MS: u64 = 60 * 60 * 1000;

/// Why one catalog entry was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogWarning {
    /// Position of the entry in the catalog file.
    pub index: usize,
    /// Its id, when it could be read.
    pub id: Option<String>,
    pub reason: String,
}

/// What the catalog screens show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSnapshot {
    pub entries: Vec<CatalogEntry>,
    /// Unix milliseconds of the last successful fetch or revalidation.
    pub fetched_at_ms: u64,
    /// `true` when the source could not be reached and this is an older copy.
    pub stale: bool,
    /// Entries dropped as invalid or duplicate.
    pub skipped: usize,
    /// Reasons of the skipped entries. Only filled right after a fetch; a
    /// snapshot read back from the cache carries the count but no reasons.
    pub warnings: Vec<CatalogWarning>,
}

impl CatalogSnapshot {
    fn from_cache(cached: CachedCatalog, stale: bool) -> Self {
        Self {
            entries: cached.entries,
            fetched_at_ms: cached.fetched_at_ms,
            stale,
            skipped: cached.skipped,
            warnings: Vec::new(),
        }
    }
}

/// Validates a downloaded catalog. A newer schema is an error; invalid,
/// unparseable and duplicate entries (the first id wins) are skipped and
/// reported as warnings.
pub fn validate_catalog(
    raw: RawCatalog,
) -> Result<(Vec<CatalogEntry>, Vec<CatalogWarning>), CatalogError> {
    if raw.schema_version > SUPPORTED_CATALOG_SCHEMA {
        return Err(CatalogError::UnsupportedSchema {
            version: raw.schema_version,
        });
    }
    let mut entries: Vec<CatalogEntry> = Vec::new();
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for (index, raw_entry) in raw.entries.into_iter().enumerate() {
        let warn = |id: Option<String>, reason: String| CatalogWarning { index, id, reason };
        match raw_entry {
            RawEntry::Unparseable { reason } => warnings.push(warn(None, reason)),
            RawEntry::Draft(draft) => {
                let raw_id = Some(draft.id.trim().to_string()).filter(|id| !id.is_empty());
                match CatalogEntry::from_draft(draft) {
                    Err(error) => warnings.push(warn(raw_id, error.to_string())),
                    Ok(entry) if !seen.insert(entry.id.clone()) => {
                        warnings.push(warn(Some(entry.id), "duplicate id".to_string()))
                    }
                    Ok(entry) => entries.push(entry),
                }
            }
        }
    }
    Ok((entries, warnings))
}

/// Strict variant for checking the catalog file shipped in the repository:
/// the schema version must be exactly the supported one and nothing may be
/// skipped. Returns every problem found.
pub fn validate_catalog_strict(raw: RawCatalog) -> Result<Vec<CatalogEntry>, Vec<String>> {
    if raw.schema_version != SUPPORTED_CATALOG_SCHEMA {
        return Err(vec![format!(
            "schemaVersion {} is not the supported {SUPPORTED_CATALOG_SCHEMA}",
            raw.schema_version
        )]);
    }
    let (entries, warnings) = validate_catalog(raw).map_err(|e| vec![e.to_string()])?;
    if warnings.is_empty() {
        return Ok(entries);
    }
    Err(warnings
        .iter()
        .map(|w| {
            let id = w.id.as_deref().unwrap_or("?");
            format!("entry {} ({id}): {}", w.index, w.reason)
        })
        .collect())
}

/// Failure bookkeeping and the result of the last completed refresh.
#[derive(Default)]
struct Flight {
    last: Option<Result<CatalogSnapshot, CatalogError>>,
    failures: u32,
    retry_at_ms: u64,
    last_error: Option<CatalogError>,
}

/// Use case: the online mod catalog, cached on disk and refreshed from the
/// source at most once per TTL, with offline fallback and backoff.
pub struct CatalogService<S, C, K> {
    source: S,
    cache: C,
    clock: K,
    ttl_ms: u64,
    /// Serializes refreshes (single-flight).
    flight: Mutex<Flight>,
    /// Bumped after every completed refresh; lets waiters reuse its result.
    generation: AtomicU64,
}

impl<S: CatalogSource, C: CatalogCache, K: Clock> CatalogService<S, C, K> {
    /// Service with the default 6 hour TTL.
    pub fn new(source: S, cache: C, clock: K) -> Self {
        Self::with_ttl(source, cache, clock, DEFAULT_CATALOG_TTL)
    }

    pub fn with_ttl(source: S, cache: C, clock: K, ttl: Duration) -> Self {
        Self {
            source,
            cache,
            clock,
            ttl_ms: u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX),
            flight: Mutex::new(Flight::default()),
            generation: AtomicU64::new(0),
        }
    }

    /// The catalog: the cache while it is fresh, otherwise a refresh. Falls
    /// back to the stale cache when the source fails.
    pub fn get(&self) -> Result<CatalogSnapshot, CatalogError> {
        if let Some(cached) = self.load_cache() {
            if self.is_fresh(&cached) {
                return Ok(CatalogSnapshot::from_cache(cached, false));
            }
        }
        self.refresh_single_flight(true)
    }

    /// Opens the web page of the entry `id` in the browser through `opener`.
    /// The entry is looked up in the cached catalog, even a stale one, without
    /// asking the source; only when nothing is cached is the catalog loaded
    /// like [`Self::get`]. The page URL must pass the domain policy.
    pub fn open_page(&self, id: &str, opener: &dyn WebPageOpener) -> Result<(), CatalogError> {
        let entries = match self.load_cache() {
            Some(cached) => cached.entries,
            None => self.get()?.entries,
        };
        let entry = entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| CatalogError::EntryNotFound { id: id.to_string() })?;
        check_page_url(&entry.page_url).map_err(|error| CatalogError::PageNotAllowed {
            id: id.to_string(),
            reason: error.to_string(),
        })?;
        opener
            .open_page(&entry.page_url)
            .map_err(|error| CatalogError::OpenFailed {
                reason: error.to_string(),
            })
    }

    /// Asks the source now, ignoring the TTL but not the backoff.
    pub fn refresh(&self) -> Result<CatalogSnapshot, CatalogError> {
        self.refresh_single_flight(false)
    }

    fn is_fresh(&self, cached: &CachedCatalog) -> bool {
        self.clock.now_millis().saturating_sub(cached.fetched_at_ms) < self.ttl_ms
    }

    /// A cache that cannot be read counts as no cache.
    fn load_cache(&self) -> Option<CachedCatalog> {
        self.cache.load().ok().flatten()
    }

    fn lock_flight(&self) -> MutexGuard<'_, Flight> {
        self.flight.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs one refresh at a time. A caller that waited while another one
    /// refreshed reuses that result instead of hitting the source again.
    fn refresh_single_flight(
        &self,
        reuse_fresh_cache: bool,
    ) -> Result<CatalogSnapshot, CatalogError> {
        let seen = self.generation.load(Ordering::SeqCst);
        let mut flight = self.lock_flight();
        if self.generation.load(Ordering::SeqCst) != seen {
            if let Some(result) = flight.last.clone() {
                return result;
            }
        }
        if reuse_fresh_cache {
            if let Some(cached) = self.load_cache() {
                if self.is_fresh(&cached) {
                    return Ok(CatalogSnapshot::from_cache(cached, false));
                }
            }
        }
        let result = self.run_refresh(&mut flight);
        flight.last = Some(result.clone());
        self.generation.fetch_add(1, Ordering::SeqCst);
        result
    }

    fn run_refresh(&self, flight: &mut Flight) -> Result<CatalogSnapshot, CatalogError> {
        let now = self.clock.now_millis();
        let cached = self.load_cache();
        if flight.failures > 0 && now < flight.retry_at_ms {
            let error = flight
                .last_error
                .clone()
                .unwrap_or(CatalogError::Unavailable {
                    reason: "waiting before retrying the catalog source".to_string(),
                });
            return Self::fall_back(cached, error);
        }
        let etag = cached.as_ref().and_then(|c| c.etag.clone());
        match self.source.fetch(etag.as_deref()) {
            Ok(SourceFetch::NotModified) => match cached {
                Some(mut cached) => {
                    cached.fetched_at_ms = now;
                    self.store(&cached);
                    self.succeeded(flight);
                    Ok(CatalogSnapshot::from_cache(cached, false))
                }
                None => {
                    let error =
                        CatalogSourceError::Invalid("not modified, but nothing is cached".into());
                    self.failed(flight, now, &error, None)
                }
            },
            Ok(SourceFetch::Fetched { catalog, etag }) => match validate_catalog(catalog) {
                Ok((entries, warnings)) => {
                    let cached = CachedCatalog {
                        fetched_at_ms: now,
                        etag,
                        entries,
                        skipped: warnings.len(),
                    };
                    self.store(&cached);
                    self.succeeded(flight);
                    let mut snapshot = CatalogSnapshot::from_cache(cached, false);
                    snapshot.warnings = warnings;
                    Ok(snapshot)
                }
                Err(error) => {
                    self.record_failure(flight, now, None, error.clone());
                    Self::fall_back(cached, error)
                }
            },
            Err(error) => self.failed(flight, now, &error, cached),
        }
    }

    /// A cache write error never fails a good fetch.
    fn store(&self, cached: &CachedCatalog) {
        let _ = self.cache.save(cached);
    }

    fn succeeded(&self, flight: &mut Flight) {
        flight.failures = 0;
        flight.retry_at_ms = 0;
        flight.last_error = None;
    }

    fn failed(
        &self,
        flight: &mut Flight,
        now: u64,
        error: &CatalogSourceError,
        cached: Option<CachedCatalog>,
    ) -> Result<CatalogSnapshot, CatalogError> {
        let retry_after = match error {
            CatalogSourceError::RateLimited {
                retry_after_secs: Some(secs),
            } => Some(*secs),
            _ => None,
        };
        let unavailable = CatalogError::Unavailable {
            reason: error.to_string(),
        };
        self.record_failure(flight, now, retry_after, unavailable.clone());
        Self::fall_back(cached, unavailable)
    }

    /// Counts the failure and schedules the next allowed attempt: the server's
    /// `Retry-After` when given, else 30 s doubling per consecutive failure.
    fn record_failure(
        &self,
        flight: &mut Flight,
        now: u64,
        retry_after_secs: Option<u64>,
        error: CatalogError,
    ) {
        flight.failures = flight.failures.saturating_add(1);
        let delay = match retry_after_secs {
            Some(secs) => secs.saturating_mul(1000),
            None => BACKOFF_BASE_MS << (flight.failures - 1).min(7),
        }
        .min(BACKOFF_MAX_MS);
        flight.retry_at_ms = now.saturating_add(delay);
        flight.last_error = Some(error);
    }

    fn fall_back(
        cached: Option<CachedCatalog>,
        error: CatalogError,
    ) -> Result<CatalogSnapshot, CatalogError> {
        match cached {
            Some(cached) => Ok(CatalogSnapshot::from_cache(cached, true)),
            None => Err(error),
        }
    }
}
