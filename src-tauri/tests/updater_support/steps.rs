//! Cucumber steps for `tests/features/updater.feature`. Every check reads the
//! real repository files; nothing about the updater plugin is simulated.
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const ENDPOINT: &str =
    "https://github.com/IcaroSilvaFK/motogp-launcher/releases/latest/download/latest.json";

#[derive(Debug, Default, World)]
struct UpdaterWorld {
    conf: Value,
    capability: Value,
    workflow: String,
    public_key: String,
    payload: Vec<u8>,
    tampered: Vec<u8>,
    signature: String,
    last_verification: Option<bool>,
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path:?}: {e}"))
}

fn fixture(name: &str) -> PathBuf {
    manifest_dir().join("tests/fixtures/updater").join(name)
}

/// Decodes the base64 wrapper that `tauri-plugin-updater` expects around
/// minisign texts (both the public key and the signature are wrapped).
fn unwrap_base64(label: &str, wrapped: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(wrapped.trim())
        .map_err(|e| format!("{label} is not valid base64: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("{label} is not UTF-8 text: {e}"))
}

/// Same chain as the plugin: unwrap both texts, decode them with minisign and
/// verify the payload. `Err` carries the reason it was refused.
fn verify(public_key: &str, signature: &str, payload: &[u8]) -> Result<(), String> {
    let key_text = unwrap_base64("public key", public_key)?;
    let key = PublicKey::decode(&key_text).map_err(|e| format!("invalid public key: {e}"))?;
    let signature_text = unwrap_base64("signature", signature)?;
    let signature =
        Signature::decode(&signature_text).map_err(|e| format!("invalid signature: {e}"))?;
    key.verify(payload, &signature, false)
        .map_err(|e| format!("verification failed: {e}"))
}

/// Pure check of `plugins.updater.pubkey`, with an actionable failure message.
fn pubkey_problem(pubkey: &str) -> Option<String> {
    let setup = "Generate the production key pair and put the PUBLIC key in \
                 plugins.updater.pubkey (see docs/auto-update.md, section setup).";
    if pubkey.is_empty() || pubkey == "REPLACE_WITH_UPDATER_PUBLIC_KEY" {
        return Some(format!(
            "plugins.updater.pubkey is still the placeholder. {setup}"
        ));
    }
    let text = match unwrap_base64("plugins.updater.pubkey", pubkey) {
        Ok(text) => text,
        Err(e) => return Some(format!("{e}. {setup}")),
    };
    if !text.starts_with("untrusted comment:") {
        return Some(format!(
            "plugins.updater.pubkey does not decode to a minisign key. {setup}"
        ));
    }
    if text.to_lowercase().contains("secret key") {
        return Some("plugins.updater.pubkey looks like a PRIVATE key: remove it NOW".into());
    }
    PublicKey::decode(&text).err().map(|e| {
        format!("plugins.updater.pubkey is not a valid minisign public key ({e}). {setup}")
    })
}

// ---------- Scenario: updater configuration ----------

#[given("the Tauri application configuration")]
fn load_conf(w: &mut UpdaterWorld) {
    w.conf = serde_json::from_str(&read(&manifest_dir().join("tauri.conf.json")))
        .expect("tauri.conf.json is valid JSON");
}

#[then("the updater endpoint is the latest release feed over HTTPS")]
fn endpoint(w: &mut UpdaterWorld) {
    let endpoints = w.conf["plugins"]["updater"]["endpoints"]
        .as_array()
        .expect("plugins.updater.endpoints must exist");
    assert_eq!(endpoints.len(), 1, "exactly one endpoint expected");
    assert_eq!(endpoints[0], ENDPOINT);
}

#[then("the updater installs in passive mode and creates updater artifacts")]
fn passive_and_artifacts(w: &mut UpdaterWorld) {
    assert_eq!(
        w.conf["plugins"]["updater"]["windows"]["installMode"],
        "passive"
    );
    assert_eq!(w.conf["bundle"]["createUpdaterArtifacts"], true);
}

#[then("the updater public key is a real minisign public key without private material")]
fn public_key_is_real(w: &mut UpdaterWorld) {
    let pubkey = w.conf["plugins"]["updater"]["pubkey"]
        .as_str()
        .unwrap_or_default();
    if let Some(problem) = pubkey_problem(pubkey) {
        panic!("{problem}");
    }
}

// ---------- Scenario: capabilities ----------

#[given("the main window capability file")]
fn load_capability(w: &mut UpdaterWorld) {
    w.capability = serde_json::from_str(&read(&manifest_dir().join("capabilities/default.json")))
        .expect("capabilities/default.json is valid JSON");
}

#[then("it grants exactly the updater check and the process restart permissions")]
fn capabilities_minimal(w: &mut UpdaterWorld) {
    let permissions = w.capability["permissions"].as_array().expect("permissions");
    let mut plain: Vec<&str> = permissions.iter().filter_map(Value::as_str).collect();
    plain.sort_unstable();
    assert_eq!(
        plain,
        [
            "core:default",
            "dialog:allow-open",
            "process:allow-restart",
            "updater:default"
        ],
        "plain permissions must stay minimal (no process:default / updater:allow-*)"
    );
    let scoped: Vec<&str> = permissions
        .iter()
        .filter_map(|p| p["identifier"].as_str())
        .collect();
    assert_eq!(scoped, ["opener:allow-open-url"]);
}

// ---------- Scenario: signature verification ----------

#[given("a test-only public key and a package signed with the matching test key")]
fn load_signature_fixtures(w: &mut UpdaterWorld) {
    w.public_key = read(&fixture("test-public-key.pub"));
    w.signature = read(&fixture("update-payload.bin.sig"));
    w.payload = fs::read(fixture("update-payload.bin")).unwrap();
    w.tampered = fs::read(fixture("update-payload-tampered.bin")).unwrap();
    assert_ne!(w.payload, w.tampered, "the fixtures must differ");
}

#[when("the signature of the untouched package is verified")]
fn verify_untouched(w: &mut UpdaterWorld) {
    w.last_verification = Some(verify(&w.public_key, &w.signature, &w.payload).is_ok());
}

#[then("the package is accepted")]
fn accepted(w: &mut UpdaterWorld) {
    assert_eq!(w.last_verification, Some(true));
}

#[when("the signature of a tampered package is verified")]
fn verify_tampered(w: &mut UpdaterWorld) {
    w.last_verification = Some(verify(&w.public_key, &w.signature, &w.tampered).is_ok());
}

#[then("the tampered package is rejected")]
fn tampered_rejected(w: &mut UpdaterWorld) {
    assert_eq!(w.last_verification, Some(false));
}

#[when("a package comes with a garbage signature or none at all")]
fn verify_unsigned(w: &mut UpdaterWorld) {
    let garbage = read(&fixture("garbage.sig"));
    let rejected = |signature: &str| verify(&w.public_key, signature, &w.payload).is_err();
    let all_rejected = rejected(&garbage) && rejected("") && rejected("%%not-base64%%");
    w.last_verification = Some(!all_rejected);
}

#[then("the unsigned package is rejected")]
fn unsigned_rejected(w: &mut UpdaterWorld) {
    assert_eq!(
        w.last_verification,
        Some(false),
        "a garbage or missing signature must be rejected"
    );
}

// ---------- Scenario: release job ----------

#[given("the validation workflow")]
fn load_workflow(w: &mut UpdaterWorld) {
    w.workflow = read(&manifest_dir().join("../.github/workflows/validacao.yml"));
}

/// Text of `build-windows` (the last job of the workflow).
fn build_job(workflow: &str) -> &str {
    let start = workflow
        .find("\n  build-windows:\n")
        .expect("job build-windows missing");
    &workflow[start..]
}

/// Text of the step whose `name:` is `name`, up to the next step.
fn step<'a>(job: &'a str, name: &str) -> &'a str {
    let marker = format!("- name: {name}\n");
    let start = job
        .find(&marker)
        .unwrap_or_else(|| panic!("step '{name}' missing"));
    let rest = &job[start + marker.len()..];
    let end = rest.find("\n      - ").unwrap_or(rest.len());
    &rest[..end]
}

#[then("the installer build needs the Rust and Frontend gates and runs only on push")]
fn build_needs_gates(w: &mut UpdaterWorld) {
    let job = build_job(&w.workflow);
    assert!(job.contains("needs: [rust, frontend]"));
    assert!(job.contains("if: ${{ github.event_name == 'push' }}"));
}

#[then("dev and qas keep the installers as 30-day artifacts without a release")]
fn dev_qas_artifacts(w: &mut UpdaterWorld) {
    let job = build_job(&w.workflow);
    let build = step(job, "Build (dev/qas)");
    assert!(build.contains("if: ${{ github.ref_name != 'main' }}"));
    assert!(
        !build.contains("uploadUpdaterJson"),
        "dev/qas must not publish a feed"
    );
    assert!(
        !build.contains("tagName"),
        "dev/qas must not create a release"
    );
    let upload = step(job, "Upload artifact");
    assert!(upload.contains("if: ${{ github.ref_name != 'main' }}"));
    assert!(upload.contains("retention-days: 30"));
    assert!(upload.contains("bundle/nsis/*.exe") && upload.contains("bundle/msi/*.msi"));
}

#[then("only main creates the release with the updater feed")]
fn only_main_releases(w: &mut UpdaterWorld) {
    let release = step(build_job(&w.workflow), "Build + Release (main)");
    assert!(release.contains("if: ${{ github.ref_name == 'main' }}"));
    assert!(release.contains("uses: tauri-apps/tauri-action@"));
    assert!(release.contains("uploadUpdaterJson: true"));
    assert!(release.contains("tagName: v__VERSION__"));
    assert_eq!(
        w.workflow.matches("uploadUpdaterJson").count(),
        1,
        "the feed is published by a single step"
    );
    assert_eq!(
        w.workflow.matches("tagName").count(),
        1,
        "a release tag exists only in the main step"
    );
}

#[then("the signing keys come only from repository secrets")]
fn keys_from_secrets(w: &mut UpdaterWorld) {
    let mut seen = 0;
    for line in w.workflow.lines() {
        if line.trim_start().starts_with("TAURI_SIGNING_PRIVATE_KEY") {
            seen += 1;
            assert!(
                line.contains("${{ secrets.TAURI_SIGNING_PRIVATE_KEY"),
                "signing value must come from secrets.*: {line}"
            );
        }
    }
    assert_eq!(seen, 4, "both build steps must receive key and password");
}

/// Runs only `updater.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = UpdaterWorld::cucumber()
        .fail_on_skipped()
        .run(manifest_dir().join("tests/features/updater.feature"))
        .await;
    !writer.execution_has_failed()
}
