use crate::{AdBeaconError, AdCacheError, AdSourceError};
use motogp_domain::{AdCreative, AdCreativeDraft};

/// One creative of a downloaded feed, before validation.
// Short-lived values, a handful per feed: boxing would only clutter the API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawAd {
    /// Parsed fields, not yet validated by the domain rules.
    Draft(AdCreativeDraft),
    /// The adapter could not even read the creative (wrong JSON shape).
    Unparseable { reason: String },
}

/// A downloaded ad feed, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdPayload {
    pub creatives: Vec<RawAd>,
}

/// Port: where the ad feed comes from (HTTPS in production, fakes in tests).
pub trait AdSource: Send + Sync {
    fn fetch(&self) -> Result<AdPayload, AdSourceError>;
}

/// A shared source is a source: lets the interface inject `Arc<dyn AdSource>`.
impl<T: AdSource + ?Sized> AdSource for std::sync::Arc<T> {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        (**self).fetch()
    }
}

/// The validated creatives as stored on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAds {
    /// Unix milliseconds of the last successful fetch.
    pub fetched_at_ms: u64,
    pub creatives: Vec<AdCreative>,
}

/// Port: persistence of the last good feed, so ads work offline.
pub trait AdCache: Send + Sync {
    /// `Ok(None)` when nothing was cached yet.
    fn load(&self) -> Result<Option<CachedAds>, AdCacheError>;
    /// Replaces the cached creatives.
    fn save(&self, ads: &CachedAds) -> Result<(), AdCacheError>;
}

/// Port: tells the sponsor an impression happened (one HTTPS GET in production).
pub trait AdBeaconSink: Send + Sync {
    fn send_impression(&self, impression_url: &str) -> Result<(), AdBeaconError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct Offline;

    impl AdSource for Offline {
        fn fetch(&self) -> Result<AdPayload, AdSourceError> {
            Err(AdSourceError::Network("offline".to_string()))
        }
    }

    #[test]
    fn a_shared_dyn_source_forwards_to_the_inner_source() {
        let shared: Arc<dyn AdSource> = Arc::new(Offline);
        assert_eq!(
            shared.fetch(),
            Err(AdSourceError::Network("offline".to_string()))
        );
    }
}
