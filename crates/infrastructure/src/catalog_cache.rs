//! `CatalogCache` backed by `catalog-cache.json` (issue #72), so the catalog
//! keeps working offline. Written atomically through the shared store helpers.
use crate::store::{load_json, save_json};
use motogp_application::{CachedCatalog, CatalogCache, CatalogCacheError};
use motogp_domain::{CatalogDownload, CatalogDownloadDraft, CatalogEntry, CatalogEntryDraft};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Format version of the cache file.
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheFile {
    version: u32,
    fetched_at_ms: u64,
    etag: Option<String>,
    entries: Vec<EntryFile>,
    skipped: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryFile {
    id: String,
    title: String,
    author: String,
    game: String,
    category: String,
    description: String,
    version: String,
    updated_at: String,
    page_url: String,
    thumbnail_url: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
    #[serde(default)]
    long_description: Option<String>,
    download: Option<DownloadFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DownloadFile {
    url: String,
    size: u64,
    sha256: String,
}

impl From<&CatalogEntry> for EntryFile {
    fn from(entry: &CatalogEntry) -> Self {
        EntryFile {
            id: entry.id.clone(),
            title: entry.title.clone(),
            author: entry.author.clone(),
            game: entry.game.clone(),
            category: entry.category.clone(),
            description: entry.description.clone(),
            version: entry.version.clone(),
            updated_at: entry.updated_at.clone(),
            page_url: entry.page_url.clone(),
            thumbnail_url: entry.thumbnail_url.clone(),
            screenshots: entry.screenshots.clone(),
            long_description: entry.long_description.clone(),
            download: entry
                .download
                .as_ref()
                .map(|CatalogDownload { url, size, sha256 }| DownloadFile {
                    url: url.clone(),
                    size: *size,
                    sha256: sha256.clone(),
                }),
        }
    }
}

impl EntryFile {
    /// Rebuilds through the domain rules; `None` when the stored entry is no longer valid.
    fn into_entry(self) -> Option<CatalogEntry> {
        CatalogEntry::from_draft(CatalogEntryDraft {
            id: self.id,
            title: self.title,
            author: self.author,
            game: self.game,
            category: self.category,
            description: self.description,
            version: self.version,
            updated_at: self.updated_at,
            page_url: self.page_url,
            thumbnail_url: self.thumbnail_url,
            screenshots: self.screenshots,
            long_description: self.long_description,
            download: self.download.map(|d| CatalogDownloadDraft {
                url: d.url,
                size: Some(d.size),
                sha256: Some(d.sha256),
            }),
        })
        .ok()
    }
}

/// JSON file implementation of [`CatalogCache`].
#[derive(Debug, Clone)]
pub struct JsonCatalogCache {
    path: PathBuf,
}

impl JsonCatalogCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl CatalogCache for JsonCatalogCache {
    /// Missing file is `Ok(None)`; unreadable, corrupt or unknown-version
    /// content is an error (the service then treats it as "no cache").
    fn load(&self) -> Result<Option<CachedCatalog>, CatalogCacheError> {
        let file = load_json::<Option<CacheFile>>(&self.path)
            .map_err(|e| CatalogCacheError(e.to_string()))?;
        let Some(file) = file else {
            return Ok(None);
        };
        if file.version != CACHE_VERSION {
            return Err(CatalogCacheError(format!(
                "unsupported cache version {}",
                file.version
            )));
        }
        let total = file.entries.len();
        let entries: Vec<CatalogEntry> = file
            .entries
            .into_iter()
            .filter_map(EntryFile::into_entry)
            .collect();
        Ok(Some(CachedCatalog {
            fetched_at_ms: file.fetched_at_ms,
            etag: file.etag,
            skipped: file.skipped + (total - entries.len()),
            entries,
        }))
    }

    fn save(&self, catalog: &CachedCatalog) -> Result<(), CatalogCacheError> {
        let file = CacheFile {
            version: CACHE_VERSION,
            fetched_at_ms: catalog.fetched_at_ms,
            etag: catalog.etag.clone(),
            entries: catalog.entries.iter().map(EntryFile::from).collect(),
            skipped: catalog.skipped,
        };
        save_json(&self.path, &file).map_err(|e| CatalogCacheError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn entry(id: &str, with_download: bool) -> CatalogEntry {
        CatalogEntry::from_draft(CatalogEntryDraft {
            id: id.into(),
            title: "Title".into(),
            author: "Author".into(),
            game: "MotoGP 26".into(),
            category: "Gameplay".into(),
            description: "Desc".into(),
            version: "1.0".into(),
            updated_at: "2026-01-02".into(),
            page_url: "https://example.com/m".into(),
            thumbnail_url: Some("https://example.com/t.png".into()),
            screenshots: vec!["https://example.com/s1.png".into()],
            long_description: Some("Long text".into()),
            download: with_download.then(|| CatalogDownloadDraft {
                url: "https://example.com/m.zip".into(),
                size: Some(42),
                sha256: Some("a".repeat(64)),
            }),
        })
        .unwrap()
    }

    fn snapshot() -> CachedCatalog {
        CachedCatalog {
            fetched_at_ms: 1_700_000_000_000,
            etag: Some("\"v1\"".into()),
            entries: vec![entry("one", true), entry("two", false)],
            skipped: 3,
        }
    }

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JsonCatalogCache::new(dir.path().join("catalog-cache.json"));
        assert_eq!(cache.load(), Ok(None));
    }

    #[test]
    fn save_then_load_roundtrips_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JsonCatalogCache::new(dir.path().join("nested").join("catalog-cache.json"));
        cache.save(&snapshot()).unwrap();
        assert_eq!(cache.load(), Ok(Some(snapshot())));
        let raw = fs::read_to_string(dir.path().join("nested/catalog-cache.json")).unwrap();
        assert!(raw.contains("\"fetchedAtMs\""));
        assert!(raw.contains("\"version\": 1"));
    }

    #[test]
    fn cache_written_before_screenshots_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog-cache.json");
        fs::write(
            &path,
            r#"{"version":1,"fetchedAtMs":1,"etag":null,"skipped":0,"entries":[{"id":"old","title":"T","author":"A","game":"G","category":"C","description":"D","version":"1","updatedAt":"2026-01-02","pageUrl":"https://example.com/m","thumbnailUrl":null,"download":null}]}"#,
        )
        .unwrap();
        let loaded = JsonCatalogCache::new(path).load().unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries[0].screenshots.is_empty());
        assert_eq!(loaded.entries[0].long_description, None);
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog-cache.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(JsonCatalogCache::new(path).load().is_err());
    }

    #[test]
    fn unknown_version_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog-cache.json");
        fs::write(
            &path,
            r#"{"version":2,"fetchedAtMs":1,"etag":null,"entries":[],"skipped":0}"#,
        )
        .unwrap();
        assert!(JsonCatalogCache::new(path).load().is_err());
    }

    #[test]
    fn invalid_cached_entries_are_dropped_and_counted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog-cache.json");
        let cache = JsonCatalogCache::new(&path);
        cache.save(&snapshot()).unwrap();
        let tampered = fs::read_to_string(&path)
            .unwrap()
            .replace("https://example.com/m\"", "http://example.com/m\"");
        fs::write(&path, tampered).unwrap();
        let loaded = cache.load().unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 0);
        assert_eq!(loaded.skipped, 3 + 2);
    }
}
