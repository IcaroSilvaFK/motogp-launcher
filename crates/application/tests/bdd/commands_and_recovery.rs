//! Steps for `tests/features/commands-and-recovery.feature` (issue #8), with
//! fake ports only. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    remove_mod, resolve_profile_mods, DeployMod, PlayError, SessionEvent, SessionState,
    TrackOutcome,
};
use motogp_domain::{Deployed, DeployedFile, LaunchError, ModEntry, Profile, WatchEvent};
use std::path::Path;

#[path = "../support/config_fakes.rs"]
mod config_fakes;
#[path = "../support/fakes.rs"]
mod library_fakes;
#[path = "../support/play_fakes.rs"]
mod play_fakes;

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
    SessionEvent { state, cleaned }
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
    w.rig().play.play("p1").expect("play is accepted afterwards");
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
            event(SessionState::Ended, 1),
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
            .all(|e| e.state == SessionState::Started));
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
    service.set_mods(&second.id, vec![w.mod_id.clone()]).unwrap();
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

/// Runs only `commands-and-recovery.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = CommandsWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("commands-and-recovery.feature"))
        .await;
    !writer.execution_has_failed()
}
