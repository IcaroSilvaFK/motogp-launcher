//! Steps for `tests/features/profiles.feature` (issue #4).
//! Fake repository and fake clock only: no fs, no real time.
//! Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, Clock, ProfileRepository, ProfileService, ProfileServiceError};
use motogp_domain::{Profile, ProfileError};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

/// The clock never advances, so rapid creations exercise the collision rule.
#[derive(Debug, Clone, Copy)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        1_700_000_000_000
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
}

impl ProfilesWorld {
    fn service(&self) -> ProfileService<SharedRepository, FixedClock> {
        ProfileService::new(self.repo.clone(), FixedClock)
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
    w.service().delete(&w.target()).unwrap();
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
    let result = w.service().delete(&w.target());
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
    assert_eq!(profile.name, "Corrida");
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

/// Runs only `profiles.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ProfilesWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("profiles.feature"))
        .await;
    !writer.execution_has_failed()
}
