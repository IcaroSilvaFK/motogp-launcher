//! `AdCache` backed by `ad-cache.json` (issue #102), so sponsored creatives keep
//! working offline. Written atomically through the shared store helpers.
//! Freshness is not decided here: `AdService` applies its TTL to `fetched_at_ms`.
use crate::store::{load_json, save_json};
use motogp_application::{AdCache, AdCacheError, CachedAds};
use motogp_domain::{AdCreative, AdCreativeDraft};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Format version of the cache file.
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheFile {
    version: u32,
    fetched_at_ms: u64,
    creatives: Vec<CreativeFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreativeFile {
    id: String,
    slot: String,
    title: String,
    body: String,
    image_data_uri: Option<String>,
    click_url: String,
    sponsor_label: String,
    impression_url: Option<String>,
    expires_at_ms: Option<u64>,
}

impl From<&AdCreative> for CreativeFile {
    fn from(creative: &AdCreative) -> Self {
        CreativeFile {
            id: creative.id().to_string(),
            slot: creative.slot().as_str().to_string(),
            title: creative.title().to_string(),
            body: creative.body().to_string(),
            image_data_uri: creative.image_data_uri().map(String::from),
            click_url: creative.click_url().to_string(),
            sponsor_label: creative.sponsor_label().to_string(),
            impression_url: creative.impression_url().map(String::from),
            expires_at_ms: creative.expires_at_ms(),
        }
    }
}

impl CreativeFile {
    /// Rebuilds through the domain rules; `None` when the stored creative is no longer valid.
    fn into_creative(self) -> Option<AdCreative> {
        AdCreative::from_draft(AdCreativeDraft {
            id: self.id,
            slot: self.slot,
            title: self.title,
            body: self.body,
            image_data_uri: self.image_data_uri,
            click_url: self.click_url,
            sponsor_label: self.sponsor_label,
            impression_url: self.impression_url,
            expires_at_ms: self.expires_at_ms,
        })
        .ok()
    }
}

/// JSON file implementation of [`AdCache`].
#[derive(Debug, Clone)]
pub struct JsonAdCache {
    path: PathBuf,
}

impl JsonAdCache {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl AdCache for JsonAdCache {
    /// Missing file is `Ok(None)`; unreadable, corrupt or unknown-version
    /// content is an error (the service then treats it as "no cache").
    fn load(&self) -> Result<Option<CachedAds>, AdCacheError> {
        let file =
            load_json::<Option<CacheFile>>(&self.path).map_err(|e| AdCacheError(e.to_string()))?;
        let Some(file) = file else {
            return Ok(None);
        };
        if file.version != CACHE_VERSION {
            return Err(AdCacheError(format!(
                "unsupported cache version {}",
                file.version
            )));
        }
        Ok(Some(CachedAds {
            fetched_at_ms: file.fetched_at_ms,
            creatives: file
                .creatives
                .into_iter()
                .filter_map(CreativeFile::into_creative)
                .collect(),
        }))
    }

    fn save(&self, ads: &CachedAds) -> Result<(), AdCacheError> {
        let file = CacheFile {
            version: CACHE_VERSION,
            fetched_at_ms: ads.fetched_at_ms,
            creatives: ads.creatives.iter().map(CreativeFile::from).collect(),
        };
        save_json(&self.path, &file).map_err(|e| AdCacheError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgo=";

    fn creative(id: &str, full: bool) -> AdCreative {
        AdCreative::from_draft(AdCreativeDraft {
            id: id.into(),
            slot: "about".into(),
            title: "Title".into(),
            body: "Body".into(),
            image_data_uri: full.then(|| PNG.to_string()),
            click_url: "https://example.com/click".into(),
            sponsor_label: "Sponsor".into(),
            impression_url: full.then(|| "https://example.com/pixel".to_string()),
            expires_at_ms: full.then_some(1_900_000_000_000),
        })
        .unwrap()
    }

    fn snapshot() -> CachedAds {
        CachedAds {
            fetched_at_ms: 1_700_000_000_000,
            creatives: vec![creative("one", true), creative("two", false)],
        }
    }

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JsonAdCache::new(dir.path().join("ad-cache.json"));
        assert_eq!(cache.load(), Ok(None));
    }

    #[test]
    fn save_then_load_roundtrips_and_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JsonAdCache::new(dir.path().join("nested").join("ad-cache.json"));
        cache.save(&snapshot()).unwrap();
        assert_eq!(cache.load(), Ok(Some(snapshot())));
        let raw = fs::read_to_string(dir.path().join("nested/ad-cache.json")).unwrap();
        assert!(raw.contains("\"fetchedAtMs\""));
        assert!(raw.contains("\"version\": 1"));
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ad-cache.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(JsonAdCache::new(path).load().is_err());
    }

    #[test]
    fn unknown_version_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ad-cache.json");
        fs::write(&path, r#"{"version":2,"fetchedAtMs":1,"creatives":[]}"#).unwrap();
        let error = JsonAdCache::new(path).load().unwrap_err();
        assert!(error.0.contains("version 2"));
    }

    #[test]
    fn unwritable_location_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        // A directory where the file should be makes the write fail.
        let path = dir.path().join("ad-cache.json");
        fs::create_dir(&path).unwrap();
        assert!(JsonAdCache::new(path).save(&snapshot()).is_err());
    }

    #[test]
    fn invalid_cached_creatives_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ad-cache.json");
        let cache = JsonAdCache::new(&path);
        cache.save(&snapshot()).unwrap();
        let tampered = fs::read_to_string(&path)
            .unwrap()
            .replace("https://example.com/pixel", "http://example.com/pixel");
        fs::write(&path, tampered).unwrap();
        let loaded = cache.load().unwrap().unwrap();
        let ids: Vec<&str> = loaded.creatives.iter().map(AdCreative::id).collect();
        assert_eq!(ids, vec!["two"]);
    }
}
