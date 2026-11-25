use crate::{AppError, CatalogCacheError, CatalogSourceError};
use motogp_domain::{CatalogEntry, CatalogEntryDraft};

/// One entry of a downloaded catalog, before validation.
// Short-lived values, a handful per catalog: boxing would only clutter the API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawEntry {
    /// Parsed fields, not yet validated by the domain rules.
    Draft(CatalogEntryDraft),
    /// The adapter could not even read the entry (wrong JSON shape).
    Unparseable { reason: String },
}

/// A downloaded catalog document, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCatalog {
    pub schema_version: u32,
    pub entries: Vec<RawEntry>,
}

/// Result of asking the source for the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFetch {
    /// The server confirmed the cached copy (the `etag` sent) is current.
    NotModified,
    /// A new document, with the server's ETag when it sent one.
    Fetched {
        catalog: RawCatalog,
        etag: Option<String>,
    },
}

/// Port: where the catalog comes from (HTTPS in production, fakes in tests).
pub trait CatalogSource: Send + Sync {
    /// Fetches the catalog. `etag` is the one of the cached copy, to allow a
    /// conditional request.
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError>;
}

/// A shared source is a source: lets the interface inject `Arc<dyn CatalogSource>`.
impl<T: CatalogSource + ?Sized> CatalogSource for std::sync::Arc<T> {
    fn fetch(&self, etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        (**self).fetch(etag)
    }
}

/// The validated catalog as stored on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCatalog {
    /// Unix milliseconds of the last successful fetch or revalidation.
    pub fetched_at_ms: u64,
    pub etag: Option<String>,
    pub entries: Vec<CatalogEntry>,
    /// Entries dropped as invalid or duplicate when the catalog was fetched.
    pub skipped: usize,
}

/// Port: persistence of the last good catalog, so the launcher works offline.
pub trait CatalogCache: Send + Sync {
    /// `Ok(None)` when nothing was cached yet.
    fn load(&self) -> Result<Option<CachedCatalog>, CatalogCacheError>;
    /// Replaces the cached catalog.
    fn save(&self, catalog: &CachedCatalog) -> Result<(), CatalogCacheError>;
}

/// Port: opens a web page in the user's default browser. Kept apart from
/// `UrlOpener` (steam links) because it carries a different trust policy.
pub trait WebPageOpener: Send + Sync {
    fn open_page(&self, url: &str) -> Result<(), AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct Offline;

    impl CatalogSource for Offline {
        fn fetch(&self, _etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
            Err(CatalogSourceError::Network("offline".to_string()))
        }
    }

    #[test]
    fn a_shared_dyn_source_forwards_to_the_inner_source() {
        let shared: Arc<dyn CatalogSource> = Arc::new(Offline);
        assert_eq!(
            shared.fetch(None),
            Err(CatalogSourceError::Network("offline".to_string()))
        );
    }
}
