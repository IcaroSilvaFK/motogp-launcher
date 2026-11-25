//! IPC contract of the online catalog commands (#72), through the Tauri mock
//! runtime with a fake source (never the network). Test names follow the
//! Gherkin scenarios of the issue (see `docs/validation-workflow.md`).
mod ipc_support;

use ipc_support::{FakeCatalogSource, Harness};
use motogp_domain::CatalogEntryDraft;
use serde_json::{json, Value};
use std::sync::Arc;

fn draft() -> CatalogEntryDraft {
    CatalogEntryDraft {
        id: "rebalanced-ai".to_string(),
        title: "Rebalanced AI".to_string(),
        author: "Someone".to_string(),
        game: "MotoGP 26".to_string(),
        category: "AI".to_string(),
        description: "Fairer riders.".to_string(),
        version: "1.2".to_string(),
        updated_at: "2026-11-01".to_string(),
        page_url: "https://example.com/rebalanced-ai".to_string(),
        ..Default::default()
    }
}

#[test]
fn get_catalog_returns_camel_case_dtos() {
    let h = Harness::with_catalog_source(Arc::new(FakeCatalogSource::serving(vec![draft()])));

    let catalog = h.invoke("get_catalog", json!({})).unwrap();

    assert_eq!(
        catalog["entries"][0]["pageUrl"],
        "https://example.com/rebalanced-ai"
    );
    assert_eq!(catalog["entries"][0]["updatedAt"], "2026-11-01");
    assert_eq!(catalog["entries"][0]["thumbnailUrl"], Value::Null);
    assert_eq!(catalog["entries"][0]["download"], Value::Null);
    assert!(catalog["fetchedAt"].as_u64().unwrap() > 0);
    assert_eq!(catalog["stale"], false);
    assert_eq!(catalog["skipped"], 0);
}

#[test]
fn unavailable_catalog_maps_to_a_stable_code() {
    let h = Harness::new();

    let error = h.invoke("refresh_catalog", json!({})).unwrap_err();

    assert_eq!(error["code"], "catalog_unavailable");
    assert!(!error["message"].as_str().unwrap().is_empty());
}

#[test]
fn refresh_catalog_caches_the_catalog_for_offline_use() {
    let h = Harness::with_catalog_source(Arc::new(FakeCatalogSource::serving(vec![draft()])));
    h.invoke("refresh_catalog", json!({})).unwrap();

    assert!(h.data_dir().join("catalog-cache.json").exists());
}

fn overtake_draft() -> CatalogEntryDraft {
    CatalogEntryDraft {
        page_url: "https://overtake.gg/downloads/rebalanced-ai.69345/".to_string(),
        ..draft()
    }
}

#[test]
fn open_catalog_page_opens_the_entry_page_by_id() {
    let h =
        Harness::with_catalog_source(Arc::new(FakeCatalogSource::serving(vec![overtake_draft()])));

    h.invoke("open_catalog_page", json!({ "id": "rebalanced-ai" }))
        .unwrap();

    assert_eq!(
        *h.pages_opened.lock().unwrap(),
        vec!["https://overtake.gg/downloads/rebalanced-ai.69345/"]
    );
}

#[test]
fn open_catalog_page_of_an_unknown_id_maps_to_a_stable_code() {
    let h =
        Harness::with_catalog_source(Arc::new(FakeCatalogSource::serving(vec![overtake_draft()])));

    let error = h
        .invoke("open_catalog_page", json!({ "id": "missing" }))
        .unwrap_err();

    assert_eq!(error["code"], "catalog_entry_not_found");
    assert!(h.pages_opened.lock().unwrap().is_empty());
}

#[test]
fn open_catalog_page_refuses_a_host_outside_the_allow_list() {
    let h = Harness::with_catalog_source(Arc::new(FakeCatalogSource::serving(vec![draft()])));

    let error = h
        .invoke("open_catalog_page", json!({ "id": "rebalanced-ai" }))
        .unwrap_err();

    assert_eq!(error["code"], "catalog_page_not_allowed");
    assert!(h.pages_opened.lock().unwrap().is_empty());
}

#[test]
fn open_catalog_page_needs_an_available_catalog() {
    let h = Harness::new();

    let error = h
        .invoke("open_catalog_page", json!({ "id": "rebalanced-ai" }))
        .unwrap_err();

    assert_eq!(error["code"], "catalog_unavailable");
}
