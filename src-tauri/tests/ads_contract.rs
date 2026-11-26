//! IPC contract of the ad commands (#103) through the Tauri mock runtime with
//! a fake feed and a controlled ad opener (never the network or a browser).
mod ipc_support;

use ipc_support::{ads_over, FakeAdSource, Harness};
use motogp_application::{AdPayload, AdSource, AdSourceError};
use motogp_domain::AdCreativeDraft;
use motogp_infrastructure::DataDir;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn creative(id: &str, slot: &str) -> AdCreativeDraft {
    AdCreativeDraft {
        id: id.to_string(),
        slot: slot.to_string(),
        title: "Acme tires".to_string(),
        body: "Grip for every lap.".to_string(),
        image_data_uri: Some("data:image/png;base64,iVBORw0KGgo=".to_string()),
        click_url: "https://sponsor.example/landing".to_string(),
        sponsor_label: "Acme".to_string(),
        ..Default::default()
    }
}

/// Counts how often the feed is asked for.
struct CountingSource {
    inner: FakeAdSource,
    fetches: Arc<AtomicUsize>,
}

impl AdSource for CountingSource {
    fn fetch(&self) -> Result<AdPayload, AdSourceError> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        self.inner.fetch()
    }
}

fn harness() -> Harness {
    Harness::with_ads(ads_over(Arc::new(FakeAdSource::serving(vec![creative(
        "acme-1", "discover",
    )]))))
}

#[test]
fn get_ad_serves_the_creative_in_camel_case_without_urls() {
    let h = harness();
    let ad = h.invoke("get_ad", json!({ "slot": "discover" })).unwrap();
    assert_eq!(
        ad,
        json!({
            "id": "acme-1",
            "title": "Acme tires",
            "body": "Grip for every lap.",
            "imageDataUri": "data:image/png;base64,iVBORw0KGgo=",
            "sponsorLabel": "Acme",
        })
    );
}

#[test]
fn an_unknown_slot_is_null_and_not_an_error() {
    let h = harness();
    let ad = h.invoke("get_ad", json!({ "slot": "banner" })).unwrap();
    assert_eq!(ad, Value::Null);
}

#[test]
fn a_slot_without_creatives_is_null() {
    let h = harness();
    let ad = h.invoke("get_ad", json!({ "slot": "sidebar" })).unwrap();
    assert_eq!(ad, Value::Null);
}

#[test]
fn ads_disabled_is_null_without_touching_the_source() {
    let fetches = Arc::new(AtomicUsize::new(0));
    let source = CountingSource {
        inner: FakeAdSource::serving(vec![creative("acme-1", "discover")]),
        fetches: fetches.clone(),
    };
    let h = Harness::with_ads(ads_over(Arc::new(source)));
    let ad = h
        .invoke("get_ad", json!({ "slot": "discover", "adsEnabled": false }))
        .unwrap();
    assert_eq!(ad, Value::Null);
    assert_eq!(fetches.load(Ordering::SeqCst), 0);
}

#[test]
fn ad_shown_with_an_unknown_id_fails_with_ad_unknown_creative() {
    let h = harness();
    let error = h
        .invoke(
            "ad_shown",
            json!({ "slot": "discover", "creativeId": "nope" }),
        )
        .unwrap_err();
    assert_eq!(error["code"], json!("ad_unknown_creative"));
}

#[test]
fn ad_shown_with_a_known_id_succeeds() {
    let h = harness();
    h.invoke(
        "ad_shown",
        json!({ "slot": "discover", "creativeId": "acme-1" }),
    )
    .unwrap();
}

#[test]
fn ad_clicked_with_an_unknown_slot_fails_without_opening_anything() {
    let h = harness();
    let error = h
        .invoke(
            "ad_clicked",
            json!({ "slot": "banner", "creativeId": "acme-1" }),
        )
        .unwrap_err();
    assert_eq!(error["code"], json!("ad_unknown_creative"));
    assert!(h.ad_pages_opened.lock().unwrap().is_empty());
}

#[test]
fn ad_clicked_rejects_snake_case_arguments() {
    let h = harness();
    assert!(h
        .invoke(
            "ad_clicked",
            json!({ "slot": "discover", "creative_id": "acme-1" })
        )
        .is_err());
    assert!(h.ad_pages_opened.lock().unwrap().is_empty());
}

#[test]
fn the_composed_state_caches_the_feed_on_disk() {
    let h = Harness::with_ad_source(Arc::new(FakeAdSource::serving(vec![creative(
        "acme-1", "discover",
    )])));
    let cache = DataDir::new(h.data_dir()).ad_cache_file();
    assert!(!cache.exists());
    h.invoke("get_ad", json!({ "slot": "discover" })).unwrap();
    assert!(cache.exists(), "the JSON ad cache was not written");
}

#[test]
fn get_ad_localizes_the_text_and_falls_back_to_the_defaults() {
    let mut localized = creative("acme-1", "discover");
    localized.image_data_uri = None;
    localized.locales.insert(
        "pt-BR".into(),
        motogp_domain::AdLocaleText {
            title: Some("Pneus Acme".into()),
            ..Default::default()
        },
    );
    let h = Harness::with_ads(ads_over(Arc::new(FakeAdSource::serving(vec![localized]))));
    let pt = h
        .invoke("get_ad", json!({ "slot": "discover", "locale": "pt-BR" }))
        .unwrap();
    assert_eq!(pt["title"], "Pneus Acme");
    assert_eq!(pt["body"], "Grip for every lap.");
    assert_eq!(pt["imageDataUri"], serde_json::Value::Null);
    let en = h
        .invoke("get_ad", json!({ "slot": "discover", "locale": "en" }))
        .unwrap();
    assert_eq!(en["title"], "Acme tires");
}
