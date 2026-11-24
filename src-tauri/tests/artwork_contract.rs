//! IPC contract of the game artwork (issue #70): `get_game_artwork` through the
//! Tauri mock runtime over a temp Steam tree, and the unchanged CSP.
mod ipc_support;

use ipc_support::Harness;
use serde_json::{json, Value};
use std::fs;

const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];

#[test]
fn get_game_artwork_is_registered_and_returns_null_without_a_game() {
    let h = Harness::new();
    assert_eq!(h.invoke("get_game_artwork", json!({})), Ok(Value::Null));
}

#[test]
fn get_game_artwork_returns_a_data_url_for_the_configured_game() {
    let h = Harness::new();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    let cache = h.tmp.path().join("steam/appcache/librarycache/123");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("header.jpg"), JPEG).unwrap();

    let url = h.invoke("get_game_artwork", json!({})).unwrap();
    let url = url.as_str().expect("a string");
    assert!(url.starts_with("data:image/jpeg;base64,"), "got: {url}");
}

#[test]
fn get_game_artwork_returns_null_when_nothing_is_cached() {
    let h = Harness::new();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    assert_eq!(h.invoke("get_game_artwork", json!({})), Ok(Value::Null));
}

#[test]
fn csp_still_only_allows_self_and_data_images() {
    let conf = include_str!("../tauri.conf.json");
    assert!(conf.contains("img-src 'self' data:"), "CSP changed");
}
