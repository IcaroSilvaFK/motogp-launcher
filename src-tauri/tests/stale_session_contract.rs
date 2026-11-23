//! IPC contract of the stale-session recovery (issue #34): `deploy_session`,
//! `forget_stale_session` and `clean_stale_session` through the Tauri mock
//! runtime, over two real temp game installs and a real `deployed.json`.
mod ipc_support;

use ipc_support::Harness;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

const OLD_FILE: &str = "000_aaaaaaaaaaaa_old.pak";

/// A game install `steam/steamapps/common/<name>` with its App ID manifest.
fn install(h: &Harness, name: &str, app_id: u32) -> PathBuf {
    let steamapps = h.tmp.path().join("steam").join("steamapps");
    let root = steamapps.join("common").join(name);
    fs::create_dir_all(root.join(name).join("Content").join("Paks")).unwrap();
    fs::write(
        steamapps.join(format!("appmanifest_{app_id}.acf")),
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"installdir\"\t\t\"{name}\"\n}}\n"
        ),
    )
    .unwrap();
    root
}

fn mods_of(root: &std::path::Path) -> PathBuf {
    let name = root.file_name().unwrap().to_string_lossy().into_owned();
    root.join(name).join("Content").join("Paks").join("~mods")
}

fn set_game(h: &Harness, root: &std::path::Path) {
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
}

/// A manifest like the one a Play on `mods` would leave: one deployed file
/// that is a byte copy of its library source, so it is provably ours.
fn deploy_manifest(h: &Harness, mods: &std::path::Path) -> PathBuf {
    fs::create_dir_all(mods).unwrap();
    let source = h.write_mod_source("old.pak", b"old-pak-content");
    fs::write(mods.join(OLD_FILE), b"old-pak-content").unwrap();
    let manifest = json!({
        "mods_dir": mods,
        "files": [{ "name": OLD_FILE, "source": source }],
        "pending": [],
    });
    fs::write(h.data_dir().join("deployed.json"), manifest.to_string()).unwrap();
    source
}

fn manifest_files(h: &Harness) -> usize {
    let raw = fs::read_to_string(h.data_dir().join("deployed.json")).unwrap();
    let manifest: Value = serde_json::from_str(&raw).unwrap();
    manifest["files"].as_array().unwrap().len()
}

/// Harness with install A configured and a manifest in A's `~mods`; then the
/// game folder is switched to B, so the manifest is stale.
struct Switched {
    h: Harness,
    old_mods: PathBuf,
    new_root: PathBuf,
}

fn switched() -> Switched {
    let h = Harness::new();
    h.recover().unwrap();
    let a = install(&h, "MotoGP25", 123);
    let b = install(&h, "MotoGP24", 456);
    set_game(&h, &a);
    let old_mods = mods_of(&a);
    deploy_manifest(&h, &old_mods);
    set_game(&h, &b);
    Switched {
        h,
        old_mods,
        new_root: b,
    }
}

fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn deploy_session_dto_has_the_documented_shape_for_every_state() {
    let h = Harness::new();
    h.recover().unwrap();
    let a = install(&h, "MotoGP25", 123);
    let b = install(&h, "MotoGP24", 456);
    set_game(&h, &a);

    let none = h.invoke("deploy_session", json!({})).unwrap();
    assert_eq!(keys(&none), ["mods_dir", "state", "tracked"]);
    assert_eq!(
        none,
        json!({ "state": "none", "mods_dir": null, "tracked": 0 })
    );

    let old_mods = mods_of(&a);
    deploy_manifest(&h, &old_mods);
    assert_eq!(
        h.invoke("deploy_session", json!({})).unwrap(),
        json!({ "state": "current", "mods_dir": null, "tracked": 1 })
    );

    set_game(&h, &b);
    assert_eq!(
        h.invoke("deploy_session", json!({})).unwrap(),
        json!({ "state": "stale", "mods_dir": old_mods.to_string_lossy(), "tracked": 1 })
    );

    fs::remove_dir_all(&old_mods).unwrap();
    assert_eq!(
        h.invoke("deploy_session", json!({})).unwrap(),
        json!({ "state": "vanished", "mods_dir": old_mods.to_string_lossy(), "tracked": 0 })
    );
}

#[test]
fn forget_stale_session_clears_the_manifest_and_keeps_the_files() {
    let s = switched();

    assert_eq!(
        s.h.invoke("forget_stale_session", json!({})).unwrap(),
        Value::Null
    );

    assert_eq!(manifest_files(&s.h), 0);
    assert!(s.old_mods.join(OLD_FILE).is_file(), "no file is deleted");
    assert_eq!(
        s.h.invoke("deploy_session", json!({})).unwrap()["state"],
        "none"
    );
}

#[test]
fn play_works_after_forgetting_the_stale_session() {
    let s = switched();
    s.h.invoke("forget_stale_session", json!({})).unwrap();

    assert_eq!(
        s.h.invoke("play", json!({ "profileId": "vanilla" }))
            .unwrap(),
        Value::Null
    );

    assert_eq!(*s.h.opened.lock().unwrap(), vec!["steam://rungameid/456"]);
    assert!(s.new_root.exists());
}

#[test]
fn clean_stale_session_accepts_the_camel_case_folder_and_removes_owned_files() {
    let s = switched();

    let cleaned =
        s.h.invoke(
            "clean_stale_session",
            json!({ "confirmedDir": s.old_mods.to_string_lossy() }),
        )
        .unwrap();

    assert_eq!(cleaned, json!(1));
    assert!(!s.old_mods.join(OLD_FILE).exists());
    assert_eq!(manifest_files(&s.h), 0);
}

#[test]
fn clean_stale_session_rejects_snake_case_arguments() {
    let s = switched();

    let error =
        s.h.invoke(
            "clean_stale_session",
            json!({ "confirmed_dir": s.old_mods.to_string_lossy() }),
        )
        .unwrap_err();

    assert!(error.to_string().contains("confirmedDir"), "{error}");
    assert!(s.old_mods.join(OLD_FILE).is_file());
}

#[test]
fn clean_stale_session_with_a_wrong_folder_is_refused_and_removes_nothing() {
    let s = switched();
    let other = s.h.tmp.path().join("elsewhere").join("~mods");

    let error =
        s.h.invoke(
            "clean_stale_session",
            json!({ "confirmedDir": other.to_string_lossy() }),
        )
        .unwrap_err();

    assert_eq!(error["code"], "deploy_unauthorized_directory");
    assert!(s.old_mods.join(OLD_FILE).is_file());
    assert_eq!(manifest_files(&s.h), 1);
}

#[test]
fn forgetting_the_current_session_is_refused_and_keeps_the_manifest() {
    let h = Harness::new();
    h.recover().unwrap();
    let a = install(&h, "MotoGP25", 123);
    set_game(&h, &a);
    let mods = mods_of(&a);
    deploy_manifest(&h, &mods);

    let error = h.invoke("forget_stale_session", json!({})).unwrap_err();

    assert_eq!(error["code"], "deploy_session_not_stale");
    assert_eq!(manifest_files(&h), 1);
    assert!(mods.join(OLD_FILE).is_file());
}

#[test]
fn recovery_is_refused_while_a_session_is_active() {
    let s = switched();
    // A session on the new install holds the lock until it ends.
    s.h.invoke("forget_stale_session", json!({})).unwrap();
    s.h.invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap();
    deploy_manifest(&s.h, &s.old_mods);

    let forget = s.h.invoke("forget_stale_session", json!({})).unwrap_err();
    let clean =
        s.h.invoke(
            "clean_stale_session",
            json!({ "confirmedDir": s.old_mods.to_string_lossy() }),
        )
        .unwrap_err();

    assert_eq!(forget["code"], "already_running");
    assert_eq!(clean["code"], "already_running");
    assert!(s.old_mods.join(OLD_FILE).is_file());
}

#[test]
fn the_recovery_commands_need_a_configured_game() {
    let h = Harness::new();
    h.recover().unwrap();

    for (cmd, body) in [
        ("deploy_session", json!({})),
        ("forget_stale_session", json!({})),
        ("clean_stale_session", json!({ "confirmedDir": "x/~mods" })),
    ] {
        let error = h.invoke(cmd, body).unwrap_err();
        assert_eq!(error["code"], "game_not_configured", "{cmd}");
    }
}
