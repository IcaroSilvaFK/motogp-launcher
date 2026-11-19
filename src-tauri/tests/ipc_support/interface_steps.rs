//! Steps for `tests/features/tauri-interface.feature` (issue #8): the real
//! commands over the Tauri mock runtime with controlled ports.
use crate::ipc_support::Harness;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use serde_json::{json, Value};
use std::fs;

#[derive(Default, World)]
struct InterfaceWorld {
    harness: Option<Harness>,
    imported: Value,
    profile: Value,
    game: Value,
    recovered: Option<usize>,
    early_error: Value,
    status: Value,
    sources: Vec<std::path::PathBuf>,
    held: Option<motogp_application::InstanceGuard>,
    lock_dir: Option<tempfile::TempDir>,
    second: Option<bool>,
}

impl std::fmt::Debug for InterfaceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InterfaceWorld")
    }
}

impl InterfaceWorld {
    fn h(&self) -> &Harness {
        self.harness.as_ref().expect("harness")
    }
}

#[given("the registered commands and controlled application ports")]
fn controlled_ports(w: &mut InterfaceWorld) {
    let h = Harness::new();
    h.recover().expect("recovery");
    w.harness = Some(h);
}

#[when("I send camelCase arguments through IPC")]
fn send_arguments(w: &mut InterfaceWorld) {
    let h = w.h();
    let root = h.create_game();
    let source = h.write_mod_source("Cool_P.pak", b"cool-pak-content");
    let game = h
        .invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
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
    let mod_id = imported[0]["id"].clone();
    let profile = h
        .invoke(
            "set_profile_mods",
            json!({ "id": profile["id"], "modIds": [mod_id] }),
        )
        .unwrap();
    h.invoke("play", json!({ "profileId": profile["id"] }))
        .unwrap();
    w.game = game;
    w.imported = imported;
    w.profile = profile;
}

#[then("each command forwards the expected operation")]
fn forwards(w: &mut InterfaceWorld) {
    let h = w.h();
    assert_eq!(*h.opened.lock().unwrap(), vec!["steam://rungameid/123"]);
    assert_eq!(h.invoke("list_mods", json!({})).unwrap(), w.imported);
    assert_eq!(h.invoke("get_game", json!({})).unwrap(), w.game);
    assert_eq!(h.runner.0.lock().unwrap().len(), 1);
}

#[then("responses use the documented snake_case fields")]
fn snake_case(w: &mut InterfaceWorld) {
    assert!(w.game.get("mods_dir").is_some() && w.game.get("app_id").is_some());
    assert!(w.imported[0].get("size_bytes").is_some());
    assert!(w.profile.get("mod_ids").is_some());
    assert!(w.profile.get("modIds").is_none());
}

#[then("errors are presented without business rules in the command")]
fn errors_presented(w: &mut InterfaceWorld) {
    let error = w
        .h()
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();
    assert_eq!(error["code"], "already_running");
    let error = w
        .h()
        .invoke("delete_profile", json!({ "id": "vanilla" }))
        .unwrap_err();
    assert_eq!(error["code"], "vanilla_protected");
}

#[given("a previous deployment left in ~mods by a crash")]
fn crash_leftover(w: &mut InterfaceWorld) {
    let h = Harness::new();
    let root = h.create_game();
    h.invoke("set_game_dir", json!({ "path": root.to_string_lossy() }))
        .unwrap();
    let library = h.data_dir().join("library").join("aaaaaaaaaaaa");
    fs::create_dir_all(&library).unwrap();
    fs::write(library.join("A.pak"), b"A.pak").unwrap();
    let mods = h.mods_dir(&root);
    fs::create_dir_all(&mods).unwrap();
    fs::write(mods.join("000_aaaaaaaaaaaa_A.pak"), b"A.pak").unwrap();
    let manifest = json!({
        "mods_dir": mods,
        "files": [{ "name": "000_aaaaaaaaaaaa_A.pak", "source": library.join("A.pak") }],
        "pending": [],
    });
    fs::write(h.data_dir().join("deployed.json"), manifest.to_string()).unwrap();
    w.early_error = h
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap_err();
    w.game = json!(mods);
    w.harness = Some(h);
}

#[when("the application setup runs")]
fn setup_runs(w: &mut InterfaceWorld) {
    w.recovered = Some(w.h().recover().expect("recovery"));
}

#[then("the recovery cleans the launcher files before the first Play is accepted")]
fn cleaned_before_play(w: &mut InterfaceWorld) {
    assert_eq!(w.early_error["code"], "recovery_pending");
    let mods = std::path::PathBuf::from(w.game.as_str().unwrap());
    assert!(!mods.join("000_aaaaaaaaaaaa_A.pak").exists());
    w.h()
        .invoke("play", json!({ "profileId": "vanilla" }))
        .unwrap();
}

#[then("the cleaned amount is recorded")]
fn amount_recorded(w: &mut InterfaceWorld) {
    assert_eq!(w.recovered, Some(1));
}

#[given("a startup recovery that failed before the frontend listened")]
fn failed_startup_recovery(w: &mut InterfaceWorld) {
    let h = Harness::new();
    fs::write(h.data_dir().join("deployed.json"), "not json").unwrap();
    h.recover().unwrap_err();
    w.harness = Some(h);
}

#[when("the frontend reads recovery_status")]
fn frontend_reads_status(w: &mut InterfaceWorld) {
    w.status = w.h().invoke("recovery_status", json!({})).unwrap();
}

#[then("it receives the failed state with the error message")]
fn receives_failed_state(w: &mut InterfaceWorld) {
    assert_eq!(w.status["state"], "failed");
    assert!(w.status["error"].is_string());
}

#[given("concurrent imports through IPC")]
fn concurrent_imports(w: &mut InterfaceWorld) {
    let h = Harness::new();
    h.recover().expect("recovery");
    w.sources = (0..4)
        .map(|n| h.write_mod_source(&format!("Mod{n}_P.pak"), format!("content {n}").as_bytes()))
        .collect();
    w.harness = Some(h);
}

#[when("the commands run on the blocking pool")]
fn commands_run_concurrently(w: &mut InterfaceWorld) {
    let h = w.h();
    std::thread::scope(|scope| {
        for source in &w.sources {
            let webview = h.webview();
            scope.spawn(move || {
                crate::ipc_support::invoke_on(
                    &webview,
                    "import_mods",
                    json!({ "paths": [source.to_string_lossy()] }),
                )
                .expect("import");
            });
        }
    });
}

#[then("every import ends up in the library index")]
fn every_import_is_indexed(w: &mut InterfaceWorld) {
    let mods = w.h().invoke("list_mods", json!({})).unwrap();
    assert_eq!(mods.as_array().unwrap().len(), 4);
}

#[given("a launcher instance holding the OS lock")]
fn instance_holds_lock(w: &mut InterfaceWorld) {
    use motogp_application::InstanceLock;
    let dir = tempfile::tempdir().unwrap();
    let lock = motogp_infrastructure::FileInstanceLock::new(dir.path().join("instance.lock"));
    w.held = lock.acquire().unwrap();
    assert!(w.held.is_some());
    w.lock_dir = Some(dir);
}

#[when("a second instance tries to acquire it")]
fn second_instance_acquires(w: &mut InterfaceWorld) {
    use motogp_application::InstanceLock;
    let path = w.lock_dir.as_ref().unwrap().path().join("instance.lock");
    w.second = Some(
        motogp_infrastructure::FileInstanceLock::new(path)
            .acquire()
            .unwrap()
            .is_some(),
    );
}

#[then("it is refused without touching the deployment state")]
fn second_instance_refused(w: &mut InterfaceWorld) {
    assert_eq!(w.second, Some(false));
    assert!(
        !w.lock_dir
            .as_ref()
            .unwrap()
            .path()
            .join("deployed.json")
            .exists(),
        "the refused instance wrote nothing"
    );
}

/// Runs only `tauri-interface.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = InterfaceWorld::cucumber()
        .fail_on_skipped()
        .run(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/features/tauri-interface.feature"),
        )
        .await;
    !writer.execution_has_failed()
}
