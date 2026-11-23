//! Steps for `tests/features/load-order.feature` (issue #55), with in-memory
//! fakes only. Traceability: docs/validation-workflow.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    list_conflicts, resolve_profile_mods, AppError, Clock, ProfileRepository, ProfileService,
};
use motogp_domain::{FileConflict, ModEntry, Profile};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::deploy_fakes as deploy;
use crate::library_fakes::{new_state, FakeRepo as FakeLibrary};

#[derive(Debug, Clone, Default)]
struct SharedProfiles(Rc<RefCell<Vec<Profile>>>);

impl ProfileRepository for SharedProfiles {
    fn load(&self) -> Result<Vec<Profile>, AppError> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, profiles: &[Profile]) -> Result<(), AppError> {
        *self.0.borrow_mut() = profiles.to_vec();
        Ok(())
    }
}

struct FixedClock;

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        1_700_000_000_000
    }
}

#[derive(Debug, Default, World)]
struct LoadOrderWorld {
    profiles: SharedProfiles,
    profile_id: String,
    library: Vec<ModEntry>,
    conflicts: Vec<FileConflict>,
    deployed: Vec<String>,
}

/// Deploy only accepts real 12-hex IDs: "a" -> "aaaaaaaaaaaa".
fn real_id(short: &str) -> String {
    short.repeat(12)
}

fn entry(id: &str) -> ModEntry {
    ModEntry {
        id: id.to_string(),
        name: id.to_string(),
        files: vec!["Shared_P.pak".to_string()],
        size_bytes: 1,
    }
}

impl LoadOrderWorld {
    fn service(&self) -> ProfileService<SharedProfiles, FixedClock> {
        ProfileService::new(self.profiles.clone(), FixedClock)
    }
}

#[given("an existing profile")]
fn existing_profile(w: &mut LoadOrderWorld) {
    w.profile_id = w.service().create("Race").unwrap().id;
}

#[when(expr = "I set its mods to {string}, {string}, {string}")]
fn set_mods(w: &mut LoadOrderWorld, a: String, b: String, c: String) {
    w.service().set_mods(&w.profile_id, vec![a, b, c]).unwrap();
}

#[then(expr = "reading it returns exactly {string}, {string}")]
fn reads_exactly(w: &mut LoadOrderWorld, a: String, b: String) {
    assert_eq!(w.service().get(&w.profile_id).unwrap().mod_ids, [a, b]);
}

#[given(expr = "a library where {string} and {string} ship the same file")]
fn same_file_library(w: &mut LoadOrderWorld, a: String, b: String) {
    w.library = vec![entry(&a), entry(&b)];
}

#[when(expr = "I list the conflicts for {string}, {string}")]
fn list(w: &mut LoadOrderWorld, a: String, b: String) {
    let state = new_state();
    state.borrow_mut().index = w.library.clone();
    w.conflicts = list_conflicts(&FakeLibrary(state), &[a, b]).unwrap();
}

#[then(expr = "I get that file with mods {string}, {string} and winner {string}")]
fn got_conflict(w: &mut LoadOrderWorld, a: String, b: String, winner: String) {
    assert_eq!(w.conflicts.len(), 1);
    assert_eq!(w.conflicts[0].path, "Shared_P.pak");
    assert_eq!(w.conflicts[0].mod_ids, [a, b]);
    assert_eq!(w.conflicts[0].winner(), winner);
}

#[given(expr = "a profile ordered {string}, {string} whose mods ship the same file")]
fn ordered_profile(w: &mut LoadOrderWorld, first: String, second: String) {
    let (first, second) = (real_id(&first), real_id(&second));
    w.library = vec![entry(&first), entry(&second)];
    let mut profile = Profile::new("p1", "Race").unwrap();
    profile.set_load_order(vec![first, second]);
    w.profiles.0.borrow_mut().push(profile);
    w.profile_id = "p1".to_string();
}

#[when("the profile is deployed")]
fn deploy_profile(w: &mut LoadOrderWorld) {
    let profile = w.service().get(&w.profile_id).unwrap();
    let mods = resolve_profile_mods(&profile, &w.library, Path::new("lib"));
    let state = deploy::new_state();
    deploy::put_sources(&state, &mods);
    w.deployed = deploy::service(&state)
        .deploy(&deploy::mods_dir(), &mods)
        .expect("deploy failed");
}

#[then(
    expr = "the file of {string} gets the prefix {string} and the file of {string} gets the prefix {string}"
)]
fn prefixes(w: &mut LoadOrderWorld, first: String, p1: String, second: String, p2: String) {
    let (first, second) = (real_id(&first), real_id(&second));
    assert!(w.deployed[0].starts_with(&format!("{p1}_{first}_")));
    assert!(w.deployed[1].starts_with(&format!("{p2}_{second}_")));
}

#[then(expr = "the winner {string} loads last")]
fn loads_last(w: &mut LoadOrderWorld, winner: String) {
    assert!(w
        .deployed
        .last()
        .unwrap()
        .contains(&format!("_{}_", real_id(&winner))));
}

/// Runs only `load-order.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = LoadOrderWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("load-order.feature"))
        .await;
    !writer.execution_has_failed()
}
