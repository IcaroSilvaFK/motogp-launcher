//! Steps for `tests/features/profile-persistence.feature` (issue #4).
//! Only temporary directories are used. Traceability: docs/workflow-validacao.md.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, ProfileRepository, ProfileService, ProfileServiceError};
use motogp_domain::Profile;
use motogp_infrastructure::{DataDir, JsonProfileStore, SystemClock};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[derive(Debug, Default, World)]
struct ProfilePersistenceWorld {
    tmp: Option<TempDir>,
    listed: Option<Result<Vec<Profile>, ProfileServiceError>>,
    reread: Option<Vec<Profile>>,
    bytes_before: Option<Vec<u8>>,
}

impl ProfilePersistenceWorld {
    fn path(&mut self) -> PathBuf {
        let tmp = self
            .tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"));
        DataDir::new(tmp.path()).profiles_file()
    }

    fn store(&mut self) -> JsonProfileStore {
        JsonProfileStore::new(self.path())
    }

    fn service(&mut self) -> ProfileService<JsonProfileStore, SystemClock> {
        ProfileService::new(self.store(), SystemClock)
    }
}

// ---------- Seeding scenarios ----------

#[given("profiles.json does not exist yet")]
fn file_missing(w: &mut ProfilePersistenceWorld) {
    assert!(!w.path().exists());
}

#[given(expr = "profiles.json contains {string}")]
fn file_contains(w: &mut ProfilePersistenceWorld, content: String) {
    let path = w.path();
    fs::write(&path, content.as_bytes()).unwrap();
    w.bytes_before = Some(content.into_bytes());
}

#[when("I list the profiles")]
fn list_profiles(w: &mut ProfilePersistenceWorld) {
    w.listed = Some(w.service().list());
}

#[then(expr = "I receive only Vanilla with ID {string} and no mods")]
fn only_vanilla(w: &mut ProfilePersistenceWorld, id: String) {
    let listed = w.listed.clone().expect("not listed").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert!(listed[0].mod_ids.is_empty());
}

#[then("profiles.json now contains Vanilla")]
fn file_contains_vanilla(w: &mut ProfilePersistenceWorld) {
    assert_eq!(w.store().load(), Ok(vec![Profile::vanilla()]));
}

// ---------- Roundtrip ----------

#[given(expr = "a saved profile {string} with mods {string}, {string}, {string}")]
fn saved_profile(w: &mut ProfilePersistenceWorld, name: String, a: String, b: String, c: String) {
    let service = w.service();
    let id = service.create(&name).unwrap().id;
    service.set_mods(&id, vec![a, b, c]).unwrap();
}

#[when("I read the profiles again from a new store")]
fn read_again(w: &mut ProfilePersistenceWorld) {
    w.reread = Some(w.store().load().unwrap());
}

#[then(expr = "the mods of that profile are {string}, {string}, {string}")]
fn mods_are(w: &mut ProfilePersistenceWorld, a: String, b: String, c: String) {
    let profiles = w.reread.clone().expect("not read");
    assert_eq!(profiles.len(), 2, "Vanilla plus the saved profile");
    assert_eq!(profiles[1].mod_ids, [a, b, c]);
}

// ---------- Invalid data is never reseeded ----------

#[then("I receive a corrupted data error")]
fn corrupted_error(w: &mut ProfilePersistenceWorld) {
    assert!(
        matches!(
            w.listed,
            Some(Err(ProfileServiceError::Storage(
                AppError::Corrupted { .. }
            )))
        ),
        "got {:?}",
        w.listed
    );
}

#[then("the original file remains intact")]
fn original_intact(w: &mut ProfilePersistenceWorld) {
    let on_disk = fs::read(w.path()).unwrap();
    assert_eq!(Some(on_disk), w.bytes_before);
}

/// Runs only `profile-persistence.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ProfilePersistenceWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("profile-persistence.feature"))
        .await;
    !writer.execution_has_failed()
}
