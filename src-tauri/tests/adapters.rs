//! Interface adapters (issue #8): event serialization, the error channel,
//! the `steam://` opener guard and the minimal capabilities.
use motogp_application::{AppError, SessionEvent, SessionPublisher, SessionState, UrlOpener};
use motogp_launcher_lib::{is_steam_url, SessionEventDto, TauriEvents, TauriOpener};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::Listener;

fn collect(app: &tauri::App<tauri::test::MockRuntime>, name: &str) -> Arc<Mutex<Vec<Value>>> {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    app.listen(name, move |event| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(event.payload()).unwrap());
    });
    seen
}

#[test]
fn session_events_serialize_with_documented_states() {
    let cases = [
        (SessionState::Started, "started"),
        (SessionState::CleaningUp, "cleaning-up"),
        (SessionState::Ended, "ended"),
        (SessionState::NeverStarted, "never-started"),
    ];
    for (state, text) in cases {
        let dto = SessionEventDto::from(SessionEvent {
            state,
            cleaned: 3,
            started_at_ms: None,
        });
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            json!({ "state": text, "cleaned": 3, "started_at": null })
        );
    }
}

#[test]
fn tauri_events_publish_session_and_report_errors_on_a_separate_channel() {
    let app = tauri::test::mock_app();
    let sessions = collect(&app, "session");
    let errors = collect(&app, "session-error");
    let events = TauriEvents::new(app.handle().clone());

    events.publish(SessionEvent {
        state: SessionState::Ended,
        cleaned: 2,
        started_at_ms: None,
    });
    events.publish(SessionEvent {
        state: SessionState::Started,
        cleaned: 0,
        started_at_ms: Some(1_700_000_000_000),
    });
    events.report_error("cleanup incomplete");

    assert_eq!(
        *sessions.lock().unwrap(),
        vec![
            json!({ "state": "ended", "cleaned": 2, "started_at": null }),
            json!({ "state": "started", "cleaned": 0, "started_at": 1_700_000_000_000u64 }),
        ]
    );
    assert_eq!(
        *errors.lock().unwrap(),
        vec![json!({ "message": "cleanup incomplete" })]
    );
}

#[test]
fn only_steam_urls_are_allowed() {
    assert!(is_steam_url("steam://rungameid/123"));
    for url in [
        "https://example.com",
        "file:///c:/windows/system32/calc.exe",
        "steam:/rungameid/1",
        "steamx://run",
        " steam://rungameid/1",
        "",
    ] {
        assert!(!is_steam_url(url), "{url:?} must be rejected");
    }
}

#[test]
fn opener_rejects_non_steam_urls_before_reaching_the_plugin() {
    let app = tauri::test::mock_app();
    let opener = TauriOpener::new(app.handle().clone());

    let error = opener.open("https://example.com").unwrap_err();

    assert!(matches!(error, AppError::Read { .. }));
    assert!(error.to_string().contains("steam://"));
}

#[test]
fn capabilities_are_minimal_and_open_only_steam_urls() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
    let capability: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let permissions = capability["permissions"].as_array().unwrap();
    let identifiers: Vec<&str> = permissions
        .iter()
        .map(|p| p.as_str().or_else(|| p["identifier"].as_str()).unwrap())
        .collect();

    let mut sorted = identifiers.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        [
            "core:default",
            // The custom title bar (issue #69).
            "core:window:allow-close",
            "core:window:allow-internal-toggle-maximize",
            "core:window:allow-minimize",
            "core:window:allow-start-dragging",
            "core:window:allow-toggle-maximize",
            "dialog:allow-open",
            "opener:allow-open-url",
            "process:allow-restart",
            "updater:default",
        ]
    );
    assert!(identifiers.contains(&"dialog:allow-open"));
    assert!(
        !identifiers.contains(&"opener:default"),
        "unrestricted opener"
    );
    for forbidden in ["shell", "process", "fs:", "http"] {
        assert!(
            identifiers
                .iter()
                .filter(|i| **i != "process:allow-restart") // the updater restart (issue #10)
                .all(|i| !i.starts_with(forbidden)),
            "{forbidden} permission is not allowed"
        );
    }
    let open_url = permissions
        .iter()
        .find(|p| p["identifier"] == "opener:allow-open-url")
        .expect("scoped open-url permission");
    assert_eq!(open_url["allow"], json!([{ "url": "steam://*" }]));
}

#[test]
fn production_csp_is_strict_and_dev_csp_only_adds_what_vite_needs() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let security = &config["app"]["security"];
    let csp = security["csp"].as_str().expect("a CSP string, not null");

    for directive in [
        "default-src 'self'",
        "script-src 'self'",
        "style-src 'self'",
        "img-src 'self' data:",
        "connect-src ipc: http://ipc.localhost",
        "object-src 'none'",
        "base-uri 'none'",
    ] {
        assert!(csp.contains(directive), "missing `{directive}` in `{csp}`");
    }
    assert!(!csp.contains("unsafe-eval"));
    assert!(!csp.contains("unsafe-inline"));
    assert!(!csp.contains('*'));
    let dev = security["devCsp"].as_str().expect("a dev CSP for Vite");
    assert!(dev.contains("ws://localhost:1420"), "HMR socket: {dev}");
    assert!(dev.contains("object-src 'none'"));
}
