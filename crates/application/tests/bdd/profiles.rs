//! Steps for `tests/features/profiles.feature` (issue #4).
//! Fake repository and fake clock only: no fs, no real time.
//! Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    AppError, Clock, ConfigRepository, ProfileRepository, ProfileService, ProfileServiceError,
    SessionActivity,
};
use motogp_domain::{Config, Profile, ProfileError};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Default)]
struct Storage {
    profiles: RefCell<Vec<Profile>>,
    saves: Cell<usize>,
}

/// Shared handle so the world can inspect what the service persisted.
#[derive(Debug, Clone, Default)]
struct SharedRepository(Rc<Storage>);

impl ProfileRepository for SharedRepository {
    fn load(&self) -> Result<Vec<Profile>, AppError> {
        Ok(self.0.profiles.borrow().clone())
    }

    fn save(&self, profiles: &[Profile]) -> Result<(), AppError> {
        self.0.saves.set(self.0.saves.get() + 1);
        *self.0.profiles.borrow_mut() = profiles.to_vec();
        Ok(())
    }
}

/// Config port that never has an active profile and accepts every save.
#[derive(Debug, Clone, Copy)]
struct NoConfig;

impl ConfigRepository for NoConfig {
    fn load(&self) -> Result<Config, AppError> {
        Ok(Config::default())
    }

    fn save(&self, _config: &Config) -> Result<(), AppError> {
        Ok(())
    }
}

/// The clock never advances, so rapid creations exercise the collision rule.
#[derive(Debug, Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        1_700_000_000_000
    }
}

/// Session flag the scenarios switch on and off.
#[derive(Debug, Default)]
struct ToggleSession(AtomicBool);

impl SessionActivity for ToggleSession {
    fn is_active(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Default, World)]
struct ProfilesWorld {
    repo: SharedRepository,
    listed: Option<Result<Vec<Profile>, ProfileServiceError>>,
    created: Option<Result<Profile, ProfileServiceError>>,
    target: Option<String>,
    errors: Vec<ProfileServiceError>,
    snapshot: Option<Vec<Profile>>,
    session: Arc<ToggleSession>,
}

impl ProfilesWorld {
    fn service(&self) -> ProfileService<SharedRepository, FixedClock> {
        ProfileService::new(self.repo.clone(), FixedClock).with_session(self.session.clone())
    }

    fn persisted(&self) -> Vec<Profile> {
        self.repo.0.profiles.borrow().clone()
    }

    fn target(&self) -> String {
        self.target.clone().expect("target profile not set")
    }

    fn create_target(&mut self, name: &str) {
        let service = self.service();
        service.list().unwrap();
        self.target = Some(service.create(name).unwrap().id);
    }
}

// ---------- Scenario: Seed missing or empty profiles ----------

#[given("there are no stored profiles")]
fn no_stored(w: &mut ProfilesWorld) {
    assert!(w.persisted().is_empty());
}

#[when("I list the profiles")]
fn list_profiles(w: &mut ProfilesWorld) {
    w.listed = Some(w.service().list());
}

#[then(expr = "I receive only Vanilla with ID {string} and no mods")]
fn only_vanilla(w: &mut ProfilesWorld, id: String) {
    let listed = w.listed.clone().expect("not listed").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0], Profile::vanilla());
    assert!(listed[0].mod_ids.is_empty());
}

#[then("that profile is persisted")]
fn vanilla_persisted(w: &mut ProfilesWorld) {
    assert_eq!(w.persisted(), vec![Profile::vanilla()]);
}

// ---------- Scenario: Create and list a profile ----------

#[given("Vanilla already exists")]
fn vanilla_exists(w: &mut ProfilesWorld) {
    w.service().list().unwrap();
    assert_eq!(w.persisted(), vec![Profile::vanilla()]);
}

#[when(expr = "I create {string}")]
fn create_profile(w: &mut ProfilesWorld, name: String) {
    w.created = Some(w.service().create(&name));
}

#[then("the list contains two profiles")]
fn two_profiles(w: &mut ProfilesWorld) {
    assert_eq!(w.service().list().unwrap().len(), 2);
}

#[then(expr = "the new profile is named {string} and has its own ID")]
fn new_profile_named(w: &mut ProfilesWorld, name: String) {
    let created = w.created.clone().expect("nothing created").unwrap();
    assert_eq!(created.name, name);
    assert!(!created.id.is_empty() && created.id != "vanilla");
    assert!(w.persisted().contains(&created));
}

// ---------- Scenario: Preserve the mod order ----------

#[given("an existing profile")]
fn existing_profile(w: &mut ProfilesWorld) {
    w.create_target("Existing");
}

#[when(expr = "I set its mods to {string}, {string}, {string}")]
fn set_mods(w: &mut ProfilesWorld, a: String, b: String, c: String) {
    w.service().set_mods(&w.target(), vec![a, b, c]).unwrap();
}

#[then(expr = "reading it returns exactly {string}, {string}, {string}")]
fn reading_returns(w: &mut ProfilesWorld, a: String, b: String, c: String) {
    assert_eq!(w.service().get(&w.target()).unwrap().mod_ids, [a, b, c]);
}

// ---------- Scenario: Delete an ordinary profile ----------

#[given("a profile other than Vanilla")]
fn profile_other_than_vanilla(w: &mut ProfilesWorld) {
    w.create_target("Ordinary");
}

#[when("I delete it")]
fn delete_it(w: &mut ProfilesWorld) {
    w.service().delete(&w.target(), &NoConfig).unwrap();
}

#[then("it no longer appears in the persisted list")]
fn no_longer_listed(w: &mut ProfilesWorld) {
    let id = w.target();
    assert!(w.persisted().iter().all(|p| p.id != id));
    assert!(w.persisted().iter().any(|p| p.is_vanilla()));
}

// ---------- Scenario: Remove references from all profiles ----------

fn to_strings(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[given(expr = "several profiles contain the mod {string}")]
fn several_contain(w: &mut ProfilesWorld, mod_id: String) {
    assert_eq!(mod_id, "a");
    let service = w.service();
    service.list().unwrap();
    for (name, mods) in [
        ("One", &["x", "a", "y"][..]),
        ("Two", &["a", "z"][..]),
        ("Three", &["y", "a"][..]),
    ] {
        let id = service.create(name).unwrap().id;
        service.set_mods(&id, to_strings(mods)).unwrap();
    }
}

#[when(expr = "I remove the references to the mod {string}")]
fn remove_references(w: &mut ProfilesWorld, mod_id: String) {
    w.service().remove_mod_everywhere(&mod_id).unwrap();
}

#[then(expr = "no profile contains {string}")]
fn no_profile_contains(w: &mut ProfilesWorld, mod_id: String) {
    assert!(w.persisted().iter().all(|p| !p.mod_ids.contains(&mod_id)));
}

#[then("the order of the other mods is preserved")]
fn others_in_order(w: &mut ProfilesWorld) {
    let all: Vec<Vec<String>> = w.persisted().into_iter().map(|p| p.mod_ids).collect();
    assert_eq!(
        all,
        vec![
            to_strings(&[]),
            to_strings(&["x", "y"]),
            to_strings(&["z"]),
            to_strings(&["y"]),
        ]
    );
}

// ---------- Scenario: Reject a blank name ----------

#[given("a name made only of spaces")]
fn blank_name(w: &mut ProfilesWorld) {
    w.create_target("Keep");
    w.snapshot = Some(w.persisted());
}

#[when("I try to create or rename a profile with that name")]
fn try_blank(w: &mut ProfilesWorld) {
    let service = w.service();
    let outcomes = [
        service.create("   ").map(|_| ()),
        service.rename(&w.target(), "  \t ").map(|_| ()),
    ];
    w.errors = outcomes.into_iter().filter_map(Result::err).collect();
    assert_eq!(w.errors.len(), 2, "both attempts must fail");
}

#[then(expr = "I receive the error {string}")]
fn receive_error(w: &mut ProfilesWorld, message: String) {
    assert!(!w.errors.is_empty());
    for error in &w.errors {
        assert_eq!(error.to_string(), message);
    }
}

#[then("the previous data is preserved")]
fn data_preserved(w: &mut ProfilesWorld) {
    assert_eq!(Some(w.persisted()), w.snapshot);
}

// ---------- Scenario: Protect Vanilla in the backend ----------

#[given(expr = "the profile with ID {string}")]
fn profile_with_id(w: &mut ProfilesWorld, id: String) {
    w.service().list().unwrap();
    assert!(w.persisted().iter().any(|p| p.id == id));
    w.target = Some(id);
}

#[when("I try to delete it through the use case")]
fn try_delete(w: &mut ProfilesWorld) {
    let result = w.service().delete(&w.target(), &NoConfig);
    w.errors = result.err().into_iter().collect();
}

#[then("the deletion is refused")]
fn deletion_refused(w: &mut ProfilesWorld) {
    assert_eq!(
        w.errors,
        vec![ProfileServiceError::Profile(ProfileError::VanillaProtected)]
    );
}

#[then("Vanilla is still saved")]
fn vanilla_still_saved(w: &mut ProfilesWorld) {
    assert_eq!(w.persisted(), vec![Profile::vanilla()]);
}

// ---------- Scenario: Rename a profile and handle an unknown ID ----------

#[when(expr = "I rename it to {string}")]
fn rename_it(w: &mut ProfilesWorld, name: String) {
    w.service().rename(&w.target(), &name).unwrap();
}

#[then("the updated name is persisted")]
fn name_persisted(w: &mut ProfilesWorld) {
    let id = w.target();
    let profile = w.persisted().into_iter().find(|p| p.id == id).unwrap();
    assert_eq!(profile.name, "Race");
}

#[when("I look up an unknown ID")]
fn lookup_unknown(w: &mut ProfilesWorld) {
    w.errors = w
        .service()
        .get("does-not-exist")
        .err()
        .into_iter()
        .collect();
}

// ---------- Scenarios: profile edits while a game session is active (#58) ----------

/// Config port that records saves, so a refused edit can prove it wrote nothing.
#[derive(Debug, Default)]
struct CountingConfig {
    saves: Cell<usize>,
}

impl ConfigRepository for CountingConfig {
    fn load(&self) -> Result<Config, AppError> {
        Ok(Config::default())
    }

    fn save(&self, _config: &Config) -> Result<(), AppError> {
        self.saves.set(self.saves.get() + 1);
        Ok(())
    }
}

#[given(expr = "a profile {string} exists")]
fn named_profile_exists(w: &mut ProfilesWorld, name: String) {
    w.create_target(&name);
    w.snapshot = Some(w.persisted());
}

#[given("a game session is active")]
fn session_active(w: &mut ProfilesWorld) {
    w.session.0.store(true, Ordering::SeqCst);
}

#[given("a game session was active and has ended")]
fn session_ended(w: &mut ProfilesWorld) {
    w.session.0.store(true, Ordering::SeqCst);
    w.session.0.store(false, Ordering::SeqCst);
}

#[when("I try to rename, delete, activate, create or change the mods of a profile")]
fn try_every_mutation(w: &mut ProfilesWorld) {
    let service = w.service();
    let id = w.target();
    let config = CountingConfig::default();
    let outcomes = [
        service.rename(&id, "Other").map(|_| ()),
        service.delete(&id, &config),
        service.set_active(&id, &config),
        service.create("Another").map(|_| ()),
        service.set_mods(&id, vec!["m1".to_string()]).map(|_| ()),
    ];
    assert_eq!(config.saves.get(), 0, "no active profile change");
    w.errors = outcomes.into_iter().filter_map(Result::err).collect();
    assert_eq!(w.errors.len(), 5, "all five attempts must fail");
}

#[then(expr = "every attempt fails with {string}")]
fn every_attempt_fails(w: &mut ProfilesWorld, kind: String) {
    assert_eq!(kind, "session active");
    assert!(w
        .errors
        .iter()
        .all(|error| *error == ProfileServiceError::SessionActive));
}

#[then("the stored profiles and the active profile are unchanged")]
fn stored_unchanged(w: &mut ProfilesWorld) {
    // The active profile lives in the config, which the counter proved untouched.
    assert_eq!(Some(w.persisted()), w.snapshot);
}

#[then(expr = "{string} is listed")]
fn profile_is_listed(w: &mut ProfilesWorld, name: String) {
    let listed = w.listed.clone().expect("not listed").unwrap();
    assert!(listed.iter().any(|p| p.name == name));
}

#[when(expr = "I change the mods of {string} to {string}")]
fn change_mods(w: &mut ProfilesWorld, _name: String, mod_id: String) {
    w.service().set_mods(&w.target(), vec![mod_id]).unwrap();
}

#[then(expr = "{string} has the mods {string}")]
fn has_mods(w: &mut ProfilesWorld, _name: String, mod_id: String) {
    assert_eq!(w.service().get(&w.target()).unwrap().mod_ids, [mod_id]);
}

/// Runs only `profiles.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ProfilesWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("profiles.feature"))
        .await;
    !writer.execution_has_failed()
}
