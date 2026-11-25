//! IPC contract tests (issue #8): every command is invoked through the Tauri
//! mock runtime with camelCase JS arguments; responses must use the documented
//! snake_case fields and failures must be presented as `{ code, message }`.
mod ipc_support;

use ipc_support::Harness;
use serde_json::{json, Value};
use std::fs;

fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn commands_preserve_camel_case_arguments_and_snake_case_dtos() {
    let h = Harness::new();
    h.recover().expect("startup recovery");
    let root = h.create_game();
    let source = h.write_mod_source("Cool_P.pak", b"cool-pak-content");

    // Configuration and game.
    let config = h.invoke("get_config", json!({})).unwrap();
    assert_eq!(config, json!({ "game_dir": null, "active_profile": null }));
    assert_eq!(h.invoke("detect_games", json!({})).unwrap(), json!([]));
    assert_eq!(h.invoke("get_game", json!({})).unwrap(), Value::Null);
    let game = h
        .invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    assert_eq!(keys(&game), ["app_id", "mods_dir", "name", "root"]);
    assert_eq!(game["app_id"], 123);
    assert_eq!(game["name"], "MotoGP25");
    assert_eq!(h.invoke("get_game", json!({})).unwrap(), game);
    let config = h.invoke("get_config", json!({})).unwrap();
    assert_eq!(config["game_dir"], game["root"]);

    // Library.
    let imported = h
        .invoke(
            "import_mods",
            json!({ "paths": [source.to_string_lossy()] }),
        )
        .unwrap();
    let entry = &imported[0];
    assert_eq!(keys(entry), ["files", "id", "name", "size_bytes"]);
    assert_eq!(entry["name"], "Cool");
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), imported);
    let mod_id = entry["id"].as_str().unwrap().to_string();

    // Profiles (camelCase `modIds`, snake_case `mod_ids` in the response).
    let created = h
        .invoke("create_profile", json!({ "name": "Race" }))
        .unwrap();
    assert_eq!(keys(&created), ["id", "mod_ids", "name"]);
    let id = created["id"].as_str().unwrap().to_string();
    let renamed = h
        .invoke("rename_profile", json!({ "id": id, "name": "Rain" }))
        .unwrap();
    assert_eq!(renamed["name"], "Rain");
    let with_mods = h
        .invoke("set_profile_mods", json!({ "id": id, "modIds": [mod_id] }))
        .unwrap();
    assert_eq!(with_mods["mod_ids"], json!([mod_id]));
    let listed = h.invoke("list_profiles", json!({})).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2, "Vanilla + Rain");
    assert_eq!(listed[0]["id"], "vanilla");
    h.invoke("set_active_profile", json!({ "id": id })).unwrap();
    let config = h.invoke("get_config", json!({})).unwrap();
    assert_eq!(config["active_profile"], json!(id));

    // Play: camelCase `profileId`; deploys, then opens Steam, session pending.
    assert_eq!(
        h.invoke("play", json!({ "profileId": id })).unwrap(),
        Value::Null
    );
    assert_eq!(*h.opened.lock().unwrap(), vec!["steam://rungameid/123"]);
    assert_eq!(fs::read_dir(h.mods_dir(&root)).unwrap().count(), 1);
    assert_eq!(
        h.runner.0.lock().unwrap().len(),
        1,
        "tracking was scheduled"
    );

    // Stale-session recovery (issue #34): the session of this Play is current;
    // the session is still active, so both recovery commands are refused.
    let session = h.invoke("deploy_session", json!({})).unwrap();
    assert_eq!(keys(&session), ["mods_dir", "state", "tracked"]);
    assert_eq!(session["state"], "current");
    assert_eq!(session["tracked"], 1);
    let forget = h.invoke("forget_stale_session", json!({})).unwrap_err();
    assert_eq!(forget["code"], "already_running");
    let clean = h
        .invoke("clean_stale_session", json!({ "confirmedDir": "x/~mods" }))
        .unwrap_err();
    assert_eq!(clean["code"], "already_running");

    // Profile mutations are refused while the session is active (issue #58).
    let locked = h.invoke("delete_profile", json!({ "id": id })).unwrap_err();
    assert_eq!(locked["code"], "profile_locked_by_session");
    let config = h.invoke("get_config", json!({})).unwrap();
    assert_eq!(config["active_profile"], json!(id));

    // Removing a mod from the library is refused too while the session is active (issue #82).
    let refused = h.invoke("remove_mod", json!({ "id": mod_id })).unwrap_err();
    assert_eq!(refused["code"], "library_locked_by_session");
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), imported);
}

#[test]
fn list_conflicts_takes_camel_case_mod_ids_and_returns_snake_case_dtos() {
    let h = Harness::new();
    h.recover().unwrap();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    // Two different mods shipping the same file name (different contents).
    let first = h.write_mod_source("Same_P.pak", b"first-pak-content");
    let imported = h
        .invoke("import_mods", json!({ "paths": [first.to_string_lossy()] }))
        .unwrap();
    let second = h.write_mod_source("same_p.pak", b"second-pak-content");
    let imported_two = h
        .invoke(
            "import_mods",
            json!({ "paths": [second.to_string_lossy()] }),
        )
        .unwrap();
    let a = imported[0]["id"].as_str().unwrap().to_string();
    let b = imported_two[0]["id"].as_str().unwrap().to_string();

    let found = h
        .invoke("list_conflicts", json!({ "modIds": [a, b] }))
        .unwrap();

    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(keys(&found[0]), ["mod_ids", "path", "winner"]);
    assert_eq!(found[0]["mod_ids"], json!([a, b]));
    assert_eq!(found[0]["winner"], json!(b));
    let reversed = h
        .invoke("list_conflicts", json!({ "modIds": [b, a] }))
        .unwrap();
    assert_eq!(reversed[0]["winner"], json!(a));
    let none = h
        .invoke("list_conflicts", json!({ "modIds": [a, "gone00000000"] }))
        .unwrap();
    assert_eq!(none, json!([]));
}

#[test]
fn list_conflicts_rejects_snake_case_arguments() {
    let h = Harness::new();
    h.recover().unwrap();

    let error = h
        .invoke("list_conflicts", json!({ "mod_ids": [] }))
        .unwrap_err();

    assert!(error.to_string().contains("modIds"), "{error}");
}

#[test]
fn snake_case_arguments_are_not_accepted_from_javascript() {
    let h = Harness::new();
    h.recover().unwrap();

    let error = h
        .invoke(
            "set_profile_mods",
            json!({ "id": "vanilla", "mod_ids": [] }),
        )
        .unwrap_err();

    assert!(error.to_string().contains("modIds"), "{error}");
}

#[test]
fn errors_are_presented_as_code_and_message_without_business_rules() {
    let h = Harness::new();
    h.recover().unwrap();

    let vanilla = h
        .invoke("delete_profile", json!({ "id": "vanilla" }))
        .unwrap_err();
    assert_eq!(vanilla["code"], "vanilla_protected");

    let no_game = h
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();
    assert_eq!(no_game["code"], "game_not_configured");
    assert!(no_game["message"].as_str().unwrap().len() > 5);

    let not_a_game = h
        .invoke(
            "set_game_dir",
            json!({ "path": h.tmp.path().to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(not_a_game["code"], "not_unreal_game");

    let unknown = h
        .invoke("set_active_profile", json!({ "id": "nope" }))
        .unwrap_err();
    assert_eq!(unknown["code"], "profile_not_found");
}

#[test]
fn concurrent_play_through_ipc_reports_the_running_game() {
    let h = Harness::new();
    h.recover().unwrap();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    h.invoke("play", json!({ "profileId": "vanilla" })).unwrap();

    let error = h
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();

    assert_eq!(error["code"], "already_running");
    assert_eq!(error["message"], "the game is already running");
    assert_eq!(h.opened.lock().unwrap().len(), 1);
}

#[test]
fn import_mods_reports_the_first_failure_and_keeps_completed_imports() {
    let h = Harness::new();
    h.recover().unwrap();
    let good = h.write_mod_source("Good_P.pak", b"good-pak-content");
    let bad = h.write_mod_source("notes.txt", b"not a mod");

    let error = h
        .invoke(
            "import_mods",
            json!({ "paths": [good.to_string_lossy(), bad.to_string_lossy()] }),
        )
        .unwrap_err();

    assert_eq!(error["code"], "unsupported_format");
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("1 import(s) completed"));
    let mods = h.invoke("list_mods", json!({})).unwrap();
    assert_eq!(
        mods.as_array().unwrap().len(),
        1,
        "the completed import remains"
    );
}

#[test]
fn startup_recovery_failure_stays_retrievable_through_recovery_status() {
    let h = Harness::new();
    assert_eq!(
        h.invoke("recovery_status", json!({})).unwrap(),
        json!({ "state": "not-run", "error": null, "cleaned_count": 0 })
    );
    // The recovery ran (and failed) before any listener could exist.
    fs::write(h.data_dir().join("deployed.json"), "not json").unwrap();
    h.recover().unwrap_err();

    let status = h.invoke("recovery_status", json!({})).unwrap();

    assert_eq!(status["state"], "failed");
    assert!(status["error"].as_str().unwrap().len() > 3);
    assert_eq!(status["cleaned_count"], 0);
    // Reading it again gives the same answer: the status is not consumed.
    assert_eq!(h.invoke("recovery_status", json!({})).unwrap(), status);
}

#[test]
fn successful_recovery_is_reported_with_its_cleaned_count() {
    let h = Harness::new();
    h.recover().unwrap();

    assert_eq!(
        h.invoke("recovery_status", json!({})).unwrap(),
        json!({ "state": "recovered", "error": null, "cleaned_count": 0 })
    );
}

#[test]
fn concurrent_imports_do_not_lose_updates() {
    let h = Harness::new();
    h.recover().unwrap();
    let sources: Vec<_> = (0..4)
        .map(|n| h.write_mod_source(&format!("Mod{n}_P.pak"), format!("content {n}").as_bytes()))
        .collect();

    std::thread::scope(|scope| {
        for source in &sources {
            let webview = h.webview();
            scope.spawn(move || {
                ipc_support::invoke_on(
                    &webview,
                    "import_mods",
                    json!({ "paths": [source.to_string_lossy()] }),
                )
                .expect("import");
            });
        }
    });

    let mods = h.invoke("list_mods", json!({})).unwrap();
    assert_eq!(mods.as_array().unwrap().len(), 4, "every import is indexed");
}

#[test]
fn new_play_errors_have_stable_codes() {
    use motogp_application::PlayError;
    use motogp_launcher_lib::CommandError;
    let cases = [
        (PlayError::GameStillRunning, "game_still_running"),
        (
            PlayError::ProcessInspection { detail: "x".into() },
            "process_inspection_failed",
        ),
        (
            PlayError::SessionThread { detail: "x".into() },
            "session_thread_failed",
        ),
    ];
    for (error, code) in cases {
        assert_eq!(CommandError::from(error).code, code);
    }
}

#[test]
fn setup_runs_recovery_before_accepting_play() {
    let h = Harness::new();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    // A previous run crashed with a mod deployed, next to a manual file.
    let library = h.data_dir().join("library").join("aaaaaaaaaaaa");
    fs::create_dir_all(&library).unwrap();
    fs::write(library.join("A.pak"), b"A.pak").unwrap();
    let mods = h.mods_dir(&root);
    fs::create_dir_all(&mods).unwrap();
    fs::write(mods.join("000_aaaaaaaaaaaa_A.pak"), b"A.pak").unwrap();
    fs::write(mods.join("manual.pak"), b"user file").unwrap();
    let manifest = json!({
        "mods_dir": mods,
        "files": [{ "name": "000_aaaaaaaaaaaa_A.pak", "source": library.join("A.pak") }],
        "pending": [],
    });
    fs::write(h.data_dir().join("deployed.json"), manifest.to_string()).unwrap();

    // Before the recovery ran, Play is refused and nothing is touched.
    let early = h
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();
    assert_eq!(early["code"], "recovery_pending");
    assert!(h.opened.lock().unwrap().is_empty());
    assert!(mods.join("000_aaaaaaaaaaaa_A.pak").exists());

    // The startup hook cleans only the launcher's files and records the count.
    assert_eq!(h.recover().unwrap(), 1);
    assert!(!mods.join("000_aaaaaaaaaaaa_A.pak").exists());
    assert!(
        mods.join("manual.pak").exists(),
        "manual files are never touched"
    );

    // Now Play is accepted.
    h.invoke("play", json!({ "profileId": "vanilla" })).unwrap();
    assert_eq!(h.opened.lock().unwrap().len(), 1);
}

#[test]
fn a_recovery_failure_is_visible_and_blocks_play() {
    let h = Harness::new();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    let elsewhere = h.tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let manifest = json!({
        "mods_dir": elsewhere,
        "files": [{ "name": "000_aaaaaaaaaaaa_A.pak", "source": "x" }],
        "pending": [],
    });
    fs::write(h.data_dir().join("deployed.json"), manifest.to_string()).unwrap();

    let error = h.recover().unwrap_err();

    assert_eq!(error["code"], "deploy_unauthorized_directory");
    assert_eq!(
        h.recorder.errors.lock().unwrap().len(),
        1,
        "visible on the error channel"
    );
    let blocked = h
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();
    assert_eq!(blocked["code"], "recovery_pending");
    assert!(h.opened.lock().unwrap().is_empty());
}

#[test]
fn session_commands_report_the_live_session_and_stop_it() {
    let h = Harness::new();
    h.recover().expect("startup recovery");
    let root = h.create_game();
    let source = h.write_mod_source("Cool_P.pak", b"cool-pak-content");
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    let imported = h
        .invoke(
            "import_mods",
            json!({ "paths": [source.to_string_lossy()] }),
        )
        .unwrap();
    let profile = h
        .invoke("create_profile", json!({ "name": "Race" }))
        .unwrap();
    h.invoke(
        "set_profile_mods",
        json!({ "id": profile["id"], "modIds": [imported[0]["id"]] }),
    )
    .unwrap();

    // Nothing runs yet: idle, and a stop is refused with its own code.
    let idle = h.invoke("current_session", json!({})).unwrap();
    assert_eq!(keys(&idle), ["phase", "started_at"]);
    assert_eq!(idle, json!({ "phase": "idle", "started_at": null }));
    let refused = h.invoke("stop_session", json!({})).unwrap_err();
    assert_eq!(refused["code"], "no_active_session");

    // Play launched the game; the (manual) worker has not seen it yet.
    h.invoke("play", json!({ "profileId": profile["id"] }))
        .unwrap();
    assert_eq!(
        h.invoke("current_session", json!({})).unwrap(),
        json!({ "phase": "starting", "started_at": null })
    );

    // Stop answers at once; the cleanup is reported through the events.
    assert_eq!(h.invoke("stop_session", json!({})).unwrap(), Value::Null);
    assert_eq!(
        h.invoke("current_session", json!({})).unwrap(),
        json!({ "phase": "stopping", "started_at": null })
    );
    assert_eq!(
        h.invoke("stop_session", json!({})).unwrap_err()["code"],
        "no_active_session",
        "a second stop while stopping is refused"
    );
}
