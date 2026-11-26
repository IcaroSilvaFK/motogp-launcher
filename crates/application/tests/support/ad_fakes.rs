//! In-memory fakes of the ad ports (issue #101). No network, no disk, no real clock.
#![allow(dead_code)]
use crate::catalog_fakes::FakeClock;
use motogp_application::{
    AdBeaconError, AdBeaconSink, AdCache, AdCacheError, AdPayload, AdService, AdSource,
    AdSourceError, CachedAds, RawAd,
};
use motogp_domain::AdCreativeDraft;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Click URL of every fixture creative built by [`creative_draft`].
pub fn click_url(id: &str) -> String {
    format!("https://sponsor.example.com/click/{id}")
}

/// Impression URL of every fixture creative built by [`creative_draft`].
pub fn impression_url(id: &str) -> String {
    format!("https://sponsor.example.com/impression/{id}")
}

pub const SPONSOR_LABEL: &str = "Acme Racing";

/// A valid creative with an image, an impression URL and no expiry.
pub fn creative_draft(id: &str, slot: &str) -> AdCreativeDraft {
    AdCreativeDraft {
        id: id.into(),
        slot: slot.into(),
        title: format!("Title of {id}"),
        body: "Grip for every lap".into(),
        image_data_uri: Some("data:image/png;base64,iVBORw0KGgo=".into()),
        click_url: click_url(id),
        sponsor_label: SPONSOR_LABEL.into(),
        impression_url: Some(impression_url(id)),
        expires_at_ms: None,
    }
}

pub fn payload(drafts: Vec<AdCreativeDraft>) -> AdPayload {
    AdPayload {
        creatives: drafts.into_iter().map(RawAd::Draft).collect(),
    }
}

struct SourceInner {
    answer: Result<AdPayload, AdSourceError>,
    calls: AtomicUsize,
}

/// Always answers the same thing and counts the fetches.
#[derive(Clone)]
pub struct FakeAdSource(Arc<SourceInner>);

impl FakeAdSource {
    pub fn serving(drafts: Vec<AdCreativeDraft>) -> Self {
        Self::answering(Ok(payload(drafts)))
    }
    pub fn failing(error: AdSourceError) -> Self {
        Self::answering(Err(error))
    }
    fn answering(answer: Result<AdPayload, AdSourceError>) -> Self {
        Self(Arc::new(SourceInner {
            answer,
            calls: AtomicUsize::new(0),
        }))
    }
    pub fn calls(&self) -> usize {
        self.0.calls.load(Ordering::SeqCst)
    }
    /// What the feed holds, for steps that validate it directly.
    pub fn payload(&self) -> AdPayload {
        self.0.answer.clone().expect("the source is failing")
    }
}

impl Default for FakeAdSource {
    fn default() -> Self {
        Self::failing(AdSourceError::NotPublished)
    }
}

impl AdSource for FakeAdSource {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        self.0.answer.clone()
    }
}

#[derive(Clone, Default)]
pub struct FakeAdCache(Arc<Mutex<Option<CachedAds>>>);

impl FakeAdCache {
    pub fn stored(&self) -> Option<CachedAds> {
        self.0.lock().unwrap().clone()
    }
}

impl AdCache for FakeAdCache {
    fn load(&self) -> Result<Option<CachedAds>, AdCacheError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, ads: &CachedAds) -> Result<(), AdCacheError> {
        *self.0.lock().unwrap() = Some(ads.clone());
        Ok(())
    }
}

/// Records the impression URLs it is asked to ping.
#[derive(Clone, Default)]
pub struct FakeBeacon(Arc<Mutex<Vec<String>>>);

impl FakeBeacon {
    pub fn sent(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl AdBeaconSink for FakeBeacon {
    fn send_impression(&self, impression_url: &str) -> Result<(), AdBeaconError> {
        self.0.lock().unwrap().push(impression_url.to_string());
        Ok(())
    }
}

pub type TestAdService = AdService<FakeAdSource, FakeAdCache, FakeBeacon, FakeClock>;
