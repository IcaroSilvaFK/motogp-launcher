//! IPC contract of the storage usage (issue #56): `get_storage_usage` and
//! `open_data_folder` through the Tauri mock runtime, over a real temp data
//! dir and a fake folder opener.
mod ipc_support;

use ipc_support::Harness;
use motogp_launcher_lib::AppState;
use serde_json::{json, Value};
use std::fs;
use std::sync::mpsc;
use std::time::Duration;
use tauri::Manager;

fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

/// Imports one mod, so `library/` and the JSON state files exist.
fn import_a_mod(h: &Harness) {
    let source = h.write_mod_source("Cool_P.pak", &[7u8; 64]);
    h.invoke(
        "import_mods",
        json!({ "paths": [source.to_string_lossy()] }),
    )
    .unwrap();
}

#[test]
fn get_storage_usage_returns_the_dto() {
    let h = Harness::new();
    import_a_mod(&h);
    fs::write(h.data_dir().join("crash.dmp"), [0u8; 5]).unwrap();

    let usage = h.invoke("get_storage_usage", json!({})).unwrap();

    assert_eq!(
        keys(&usage),
        ["categories", "free_bytes", "root", "total_bytes"]
    );
    assert_eq!(usage["root"], h.data_dir().to_string_lossy().as_ref());
    let categories = usage["categories"].as_array().unwrap();
    let names: Vec<&str> = categories
        .iter()
        .map(|c| c["category"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["mods", "launcher_data", "other"]);
    assert_eq!(keys(&categories[0]), ["bytes", "category"]);
    assert!(categories[0]["bytes"].as_u64().unwrap() >= 64, "{usage}");
    assert!(categories[1]["bytes"].as_u64().unwrap() > 0, "{usage}");
    assert_eq!(categories[2]["bytes"], 5);
    let sum: u64 = categories
        .iter()
        .map(|c| c["bytes"].as_u64().unwrap())
        .sum();
    assert_eq!(usage["total_bytes"], sum);
    assert!(usage["free_bytes"].as_u64().unwrap() > 0);
}

#[test]
fn open_data_folder_opens_only_the_data_folder() {
    let h = Harness::new();

    // No argument exists: whatever the frontend sends is ignored.
    h.invoke("open_data_folder", json!({ "path": "C:/Windows" }))
        .unwrap();

    assert_eq!(*h.folders_opened.lock().unwrap(), vec![h.data_dir()]);
}

#[test]
fn measuring_does_not_wait_for_a_running_mutation() {
    let h = Harness::new();
    let state = h.app.state::<AppState>();
    let _mutation = state.write_lock.lock().unwrap();
    let webview = h.webview();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        sender
            .send(ipc_support::invoke_on(
                &webview,
                "get_storage_usage",
                json!({}),
            ))
            .ok();
    });

    let usage = receiver
        .recv_timeout(Duration::from_secs(20))
        .expect("the usage must not wait for the write lock");

    assert!(usage.is_ok(), "{usage:?}");
}
