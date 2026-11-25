//! Steps for `tests/features/commands-and-recovery.feature` (issue #8), with
//! fake ports only. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    remove_mod, resolve_profile_mods, DeployMod, PlayError, RemoveModError, SessionActivity,
    SessionEvent, SessionState, TrackOutcome,
};
use motogp_domain::{Deployed, DeployedFile, LaunchError, ModEntry, Profile, WatchEvent};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::{config_fakes, library_fakes, play_fakes};

use play_fakes::deploy::{disk_has, disk_names, put_disk, put_source, ID_A};
use play_fakes::{game, Rig, DEPLOYED_A};

#[derive(Default, World)]
struct CommandsWorld {
    profile: Option<Profile>,
    library: Vec<ModEntry>,
    resolved: Vec<DeployMod>,
    rig: Option<Rig>,
    extra_rigs: Vec<Rig>,
    errors: Vec<PlayError>,
    recovered: Option<usize>,
    mod_id: String,
    parts: Option<ProfileParts>,
    session: Arc<ToggleSession>,
    library_state: Option<library_fakes::Shared>,
    removal: Option<Result<(), RemoveModError>>,
}

impl std::fmt::Debug for CommandsWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommandsWorld")
    }
}

impl CommandsWorld {
    fn rig(&self) -> &Rig {
        self.rig.as_ref().expect("rig")
    }
}

fn entry(id: &str, name: &str) -> ModEntry {
    ModEntry {
        id: id.into(),
        name: name.into(),
        files: vec![format!("{name}.pak")],
        size_bytes: 1,
    }
}

fn seed_manifest(rig: &Rig) {
    let source = put_source(&rig.disk, ID_A, "A.pak", b"A.pak");
    put_disk(&rig.disk, DEPLOYED_A, b"A.pak");
    rig.disk.borrow_mut().manifest = Deployed {
        mods_dir: play_fakes::deploy::mods_dir(),
        files: vec![DeployedFile::new(DEPLOYED_A, source)],
        pending: vec![],
    };
}

fn event(state: SessionState, cleaned: usize) -> SessionEvent {
    let started_at_ms = (state == SessionState::Started).then_some(1_700_000_000_000);
    SessionEvent {
        state,
        cleaned,
        started_at_ms,
    }
}

// ---------- Resolve ----------

#[given(regex = r#"^a profile with IDs "b", "gone", "a" and a library with A and B$"#)]
fn profile_with_ids(w: &mut CommandsWorld) {
    let mut profile = Profile::new("p1", "Race").unwrap();
    profile.mod_ids = vec!["b".into(), "gone".into(), "a".into()];
    w.profile = Some(profile);
    w.library = vec![entry("a", "A"), entry("b", "B")];
}

#[when("I resolve the profile mods")]
fn resolve(w: &mut CommandsWorld) {
    let profile = w.profile.as_ref().unwrap();
    w.resolved = resolve_profile_mods(profile, &w.library, Path::new("lib"));
}

#[then("I get B and A in that order")]
fn b_then_a(w: &mut CommandsWorld) {
    let ids: Vec<&str> = w.resolved.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "a"]);
}

// ---------- Recovery ----------

#[given("a pending manifest and a manual file in ~mods")]
fn pending_manifest(w: &mut CommandsWorld) {
    let rig = Rig::new();
    seed_manifest(&rig);
    put_disk(&rig.disk, "manual.pak", b"user file");
    w.rig = Some(rig);
}

#[when("the launcher starts")]
fn launcher_starts(w: &mut CommandsWorld) {
    w.recovered = Some(w.rig().play.recover_on_startup().expect("recovery"));
}

#[then("the recovery cleans only the files deployed by the launcher")]
fn recovery_cleans_only_ours(w: &mut CommandsWorld) {
    assert_eq!(disk_names(&w.rig().disk), vec!["manual.pak"]);
}

#[then("records the amount removed before accepting a new session")]
fn records_amount(w: &mut CommandsWorld) {
    assert_eq!(w.recovered, Some(1));
    assert_eq!(w.rig().play.recovered_count(), Some(1));
    w.rig()
        .play
        .play("p1")
        .expect("play is accepted afterwards");
}

// ---------- Concurrent session ----------

#[given("a session starting or running")]
fn session_running(w: &mut CommandsWorld) {
    let rig = Rig::ready();
    rig.play.play("p1").expect("first play");
    w.rig = Some(rig);
}

#[when("I receive another Play request")]
fn another_play(w: &mut CommandsWorld) {
    w.errors.push(w.rig().play.play("p1").unwrap_err());
}

#[then(regex = r#"^I return "the game is already running"$"#)]
fn already_running(w: &mut CommandsWorld) {
    assert_eq!(w.errors[0].to_string(), "the game is already running");
}

#[then("I neither deploy nor open the game again")]
fn no_second_deploy(w: &mut CommandsWorld) {
    assert_eq!(w.rig().launches(), 1);
    assert_eq!(w.rig().materialized(), 1);
}

// ---------- Prerequisites ----------

#[given("a game not configured or without an App ID")]
fn prerequisites_missing(w: &mut CommandsWorld) {
    w.rig = Some(Rig::ready());
}

#[when("I receive the Play request")]
fn play_request(w: &mut CommandsWorld) {
    let rig = w.rig();
    rig.set_game(Err(PlayError::GameNotConfigured));
    let first = rig.play.play("p1").unwrap_err();
    rig.set_game(Ok(game(None)));
    let second = rig.play.play("p1").unwrap_err();
    w.errors = vec![first, second];
}

#[then("I return the corresponding error")]
fn corresponding_error(w: &mut CommandsWorld) {
    assert_eq!(
        w.errors,
        vec![
            PlayError::GameNotConfigured,
            PlayError::Launch(LaunchError::MissingAppId)
        ]
    );
}

#[then("no file or directory is created in ~mods")]
fn nothing_created(w: &mut CommandsWorld) {
    let disk = w.rig().disk.borrow();
    assert!(disk.dirs.is_empty());
    assert!(disk.disk.is_empty());
    assert_eq!(disk.manifest_saves, 0);
}

#[then("a future attempt remains possible")]
fn future_attempt(w: &mut CommandsWorld) {
    w.rig().set_game(Ok(game(Some(1))));
    w.rig().play.play("p1").expect("a later attempt works");
}

// ---------- Finish a session ----------

#[given("a session with deployed files")]
fn session_with_files(w: &mut CommandsWorld) {
    let rig = Rig::ready();
    rig.play.play("p1").expect("play");
    assert!(disk_has(&rig.disk, DEPLOYED_A));
    w.rig = Some(rig);
}

#[when("the watcher signals Ended or NeverStarted")]
fn watcher_finishes(w: &mut CommandsWorld) {
    let rig = w.rig();
    rig.runner.run_pending();
    *rig.runtime.events.lock().unwrap() = vec![WatchEvent::NeverStarted];
    rig.play.play("p1").expect("second session");
    rig.runner.run_pending();
}

#[then("the cleanup runs")]
fn cleanup_runs(w: &mut CommandsWorld) {
    assert!(disk_names(&w.rig().disk).is_empty());
    assert!(w.rig().disk.borrow().manifest.is_empty());
}

#[then("the session event contains the state and the amount actually removed")]
fn event_has_count(w: &mut CommandsWorld) {
    assert_eq!(
        w.rig().events(),
        vec![
            event(SessionState::Started, 0),
            SessionEvent {
                state: SessionState::CleaningUp,
                cleaned: 0,
                started_at_ms: Some(1_700_000_000_000),
            },
            event(SessionState::Ended, 1),
            SessionEvent {
                state: SessionState::CleaningUp,
                cleaned: 0,
                started_at_ms: None,
            },
            event(SessionState::NeverStarted, 1),
        ]
    );
}

#[then("a new session can be started after the complete cleanup")]
fn new_session_possible(w: &mut CommandsWorld) {
    w.rig().play.play("p1").expect("lock released");
}

// ---------- Started ----------

#[given("a session with deployed mods")]
fn session_with_mods(w: &mut CommandsWorld) {
    let rig = Rig::ready();
    *rig.runtime.events.lock().unwrap() = vec![WatchEvent::Started];
    *rig.runtime.outcome.lock().unwrap() = Ok(TrackOutcome::Cancelled);
    rig.play.play("p1").expect("play");
    w.rig = Some(rig);
}

#[when("the watcher signals Started")]
fn watcher_started(w: &mut CommandsWorld) {
    w.rig().runner.run_pending();
}

#[then(regex = r#"^I emit session with state "started"$"#)]
fn emit_started(w: &mut CommandsWorld) {
    assert_eq!(w.rig().events(), vec![event(SessionState::Started, 0)]);
}

#[then("the files remain deployed")]
fn files_remain(w: &mut CommandsWorld) {
    assert!(disk_has(&w.rig().disk, DEPLOYED_A));
}

// ---------- Opener failure ----------

#[given("mods deployed for a session attempt")]
fn attempt(w: &mut CommandsWorld) {
    let rig = Rig::ready();
    put_disk(&rig.disk, "manual.pak", b"user file");
    *rig.runtime.launch_result.lock().unwrap() = Err(motogp_application::SessionError::Opener(
        motogp_application::AppError::Read {
            location: "steam://rungameid/1".into(),
            detail: "no handler".into(),
        },
    ));
    w.rig = Some(rig);
}

#[when("the opener adapter returns an error")]
fn opener_fails(w: &mut CommandsWorld) {
    w.errors.push(w.rig().play.play("p1").unwrap_err());
}

#[then("I try to clean only the files of that session")]
fn clean_only_session(w: &mut CommandsWorld) {
    assert_eq!(disk_names(&w.rig().disk), vec!["manual.pak"]);
    assert!(w.rig().disk.borrow().manifest.is_empty());
}

#[then("I report the error without keeping the lock after a successful compensation")]
fn report_and_release(w: &mut CommandsWorld) {
    assert!(w.errors[0].to_string().contains("no handler"));
    *w.rig().runtime.launch_result.lock().unwrap() = Ok(());
    w.rig().play.play("p1").expect("lock released");
}

// ---------- Recovery failure ----------

#[given("the manifest cleanup fails on startup or at the end")]
fn cleanup_fails(w: &mut CommandsWorld) {
    let startup = Rig::new();
    seed_manifest(&startup);
    startup.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    let ending = Rig::ready();
    ending.play.play("p1").expect("play");
    ending.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    w.rig = Some(startup);
    w.extra_rigs = vec![ending];
}

#[when("the operation returns")]
fn operation_returns(w: &mut CommandsWorld) {
    assert!(w.rig().play.recover_on_startup().is_err());
    w.extra_rigs[0].runner.run_pending();
}

#[then("I report the failure and retain the recoverable pending items")]
fn failure_reported(w: &mut CommandsWorld) {
    for rig in std::iter::once(w.rig()).chain(w.extra_rigs.iter()) {
        assert_eq!(rig.errors().len(), 1);
        assert_eq!(rig.disk.borrow().manifest.files.len(), 1);
    }
}

#[then("I do not announce a clean session nor allow an unsafe new deploy")]
fn no_false_success(w: &mut CommandsWorld) {
    for rig in std::iter::once(w.rig()).chain(w.extra_rigs.iter()) {
        assert!(rig
            .events()
            .iter()
            .all(|e| !matches!(e.state, SessionState::Ended | SessionState::NeverStarted)));
        let before = rig.materialized();
        assert!(matches!(
            rig.play.play("p1").unwrap_err(),
            PlayError::RecoveryPending { .. }
        ));
        assert_eq!(rig.materialized(), before);
    }
}

// ---------- Profile consistency ----------

type ProfileParts = (
    library_fakes::FakeService,
    motogp_application::ProfileService<config_fakes::FakeProfiles, config_fakes::FixedClock>,
    config_fakes::FakeProfiles,
    config_fakes::FakeConfig,
    String,
);

#[given("a mod referenced in profiles and a regular active profile")]
fn mod_in_profiles(w: &mut CommandsWorld) {
    let state = library_fakes::new_state();
    library_fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    let mods = library_fakes::service(&state);
    w.mod_id = mods.import(Path::new("src/A.pak")).unwrap()[0].id.clone();
    let profiles = config_fakes::FakeProfiles::default();
    let config = config_fakes::FakeConfig::default();
    let service = config_fakes::profile_service(&profiles);
    let first = service.create("One").unwrap();
    let second = service.create("Two").unwrap();
    service.set_mods(&first.id, vec![w.mod_id.clone()]).unwrap();
    service
        .set_mods(&second.id, vec![w.mod_id.clone()])
        .unwrap();
    service.set_active(&second.id, &config).unwrap();
    w.parts = Some((mods, service, profiles, config, second.id));
}

#[when("I remove the mod and delete the active profile")]
fn remove_and_delete(w: &mut CommandsWorld) {
    let (mods, service, _, config, active) = w.parts.as_ref().unwrap();
    remove_mod(mods, service, &w.mod_id).unwrap();
    service.delete(active, config).unwrap();
}

#[then("the references to the mod are removed from all profiles")]
fn references_removed(w: &mut CommandsWorld) {
    let (_, _, profiles, _, _) = w.parts.as_ref().unwrap();
    assert!(profiles
        .0
        .borrow()
        .iter()
        .all(|p| !p.mod_ids.contains(&w.mod_id)));
}

#[then("the active profile becomes Vanilla")]
fn becomes_vanilla(w: &mut CommandsWorld) {
    let (_, _, _, config, _) = w.parts.as_ref().unwrap();
    assert_eq!(
        config.0.borrow().config.active_profile.as_deref(),
        Some(motogp_domain::VANILLA_ID)
    );
}

#[given("a manifest left by a previous run and its game still running")]
fn manifest_with_running_game(w: &mut CommandsWorld) {
    let rig = Rig::new();
    seed_manifest(&rig);
    *rig.runtime.running.lock().unwrap() = Ok(true);
    w.rig = Some(rig);
}

#[then("the files stay deployed and tracking resumes")]
fn files_stay_and_tracking_resumes(w: &mut CommandsWorld) {
    assert!(disk_has(&w.rig().disk, DEPLOYED_A));
    assert_eq!(w.rig().runner.pending(), 1);
}

#[then("Play is refused until the game ends and the cleanup completes")]
fn play_refused_until_the_game_ends(w: &mut CommandsWorld) {
    let rig = w.rig();
    assert_eq!(
        rig.play.play("p1").unwrap_err(),
        PlayError::GameStillRunning
    );
    rig.runner.run_pending();
    assert!(disk_names(&rig.disk).is_empty());
    *rig.runtime.running.lock().unwrap() = Ok(false);
    rig.play.play("p1").expect("play after the game ended");
}

#[given("a session whose background worker panics")]
fn session_with_panicking_worker(w: &mut CommandsWorld) {
    let rig = Rig::ready();
    rig.runtime
        .track_panics
        .store(true, std::sync::atomic::Ordering::SeqCst);
    rig.play.play("p1").expect("play");
    w.rig = Some(rig);
}

#[when("the worker runs")]
fn worker_runs(w: &mut CommandsWorld) {
    let rig = w.rig();
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rig.runner.run_pending()));
    assert!(outcome.is_err(), "the worker panicked");
}

#[then("the lock is released and the cleanup stays pending")]
fn lock_released_cleanup_pending(w: &mut CommandsWorld) {
    use motogp_application::SessionLock;
    let rig = w.rig();
    assert!(rig.lock.try_acquire(), "the lock was released");
    rig.lock.release();
    assert!(disk_has(&rig.disk, DEPLOYED_A), "files stay deployed");
}

#[then("the next Play retries the cleanup before deploying")]
fn next_play_retries_cleanup(w: &mut CommandsWorld) {
    let rig = w.rig();
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    assert!(matches!(
        rig.play.play("p1").unwrap_err(),
        PlayError::RecoveryPending { .. }
    ));
}

#[given("a manifest whose cleanup fails on startup")]
fn manifest_with_failing_cleanup(w: &mut CommandsWorld) {
    let rig = Rig::new();
    seed_manifest(&rig);
    rig.disk.borrow_mut().fail_remove = vec![DEPLOYED_A.to_string()];
    w.errors.push(rig.play.recover_on_startup().unwrap_err());
    w.rig = Some(rig);
}

#[when("the interface reads the recovery status after subscribing")]
fn interface_reads_status(_w: &mut CommandsWorld) {}

#[then("it still sees the failure and its message")]
fn status_keeps_the_failure(w: &mut CommandsWorld) {
    let status = w.rig().play.recovery_status();
    assert_eq!(status.state, motogp_application::RecoveryState::Failed);
    assert!(status.error.unwrap().contains("cleanup incomplete"));
}

// ---------- Scenarios: removing a mod while a game session is active (#82) ----------

/// Session flag the scenarios switch on and off.
#[derive(Debug, Default)]
struct ToggleSession(AtomicBool);

impl SessionActivity for ToggleSession {
    fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[given(expr = "the library has the mod {string} referenced by the profile {string}")]
fn library_mod_in_profile(w: &mut CommandsWorld, _mod_name: String, profile: String) {
    let state = library_fakes::new_state();
    library_fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    let mods = library_fakes::service(&state);
    w.mod_id = mods.import(Path::new("src/A.pak")).unwrap()[0].id.clone();
    let profiles = config_fakes::FakeProfiles::default();
    let config = config_fakes::FakeConfig::default();
    // The session flag is still idle while the fixtures are built.
    let service = config_fakes::profile_service(&profiles).with_session(w.session.clone());
    let created = service.create(&profile).unwrap();
    service
        .set_mods(&created.id, vec![w.mod_id.clone()])
        .unwrap();
    w.parts = Some((mods, service, profiles, config, created.id));
    w.library_state = Some(state);
}

#[given("a game session is active")]
fn session_active(w: &mut CommandsWorld) {
    w.session.0.store(true, Ordering::SeqCst);
}

#[given("a game session was active and has ended")]
fn session_ended(w: &mut CommandsWorld) {
    w.session.0.store(true, Ordering::SeqCst);
    w.session.0.store(false, Ordering::SeqCst);
}

#[when(expr = "I remove the mod {string}")]
fn remove_the_mod(w: &mut CommandsWorld, _mod_name: String) {
    let (mods, service, _, _, _) = w.parts.as_ref().unwrap();
    w.removal = Some(remove_mod(mods, service, &w.mod_id));
}

#[then(expr = "the removal fails with {string}")]
fn removal_fails(w: &mut CommandsWorld, kind: String) {
    assert_eq!(kind, "session active");
    assert_eq!(w.removal, Some(Err(RemoveModError::SessionActive)));
}

fn library_ids(w: &CommandsWorld) -> Vec<String> {
    let state = w.library_state.as_ref().expect("library state");
    let ids = state.borrow().index.iter().map(|m| m.id.clone()).collect();
    ids
}

#[then(expr = "{string} is still in the library")]
fn still_in_library(w: &mut CommandsWorld, _mod_name: String) {
    assert_eq!(library_ids(w), vec![w.mod_id.clone()]);
}

#[then(expr = "{string} is no longer in the library")]
fn no_longer_in_library(w: &mut CommandsWorld, _mod_name: String) {
    assert_eq!(w.removal, Some(Ok(())));
    assert!(library_ids(w).is_empty());
}

#[then(expr = "{string} still has the mod {string}")]
fn profile_still_has_mod(w: &mut CommandsWorld, _profile: String, _mod_name: String) {
    let (_, service, _, _, profile_id) = w.parts.as_ref().unwrap();
    assert_eq!(
        service.get(profile_id).unwrap().mod_ids,
        vec![w.mod_id.clone()]
    );
}

#[then(expr = "{string} has no mods")]
fn profile_has_no_mods(w: &mut CommandsWorld, _profile: String) {
    let (_, service, _, _, profile_id) = w.parts.as_ref().unwrap();
    assert!(service.get(profile_id).unwrap().mod_ids.is_empty());
}

/// Runs only `commands-and-recovery.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CommandsWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("commands-and-recovery.feature"))
        .await;
    !writer.execution_has_failed()
}
