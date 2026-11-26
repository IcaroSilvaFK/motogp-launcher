//! Use-case tests of the catalog install with in-memory fakes (issue #74).
#[path = "support/catalog_fakes.rs"]
pub mod catalog_fakes;
#[path = "support/config_fakes.rs"]
pub mod config_fakes;
#[path = "support/install_fakes.rs"]
mod install_fakes;
#[path = "support/fakes.rs"]
pub mod library_fakes;

use install_fakes::*;
use library_fakes::{fake_hash, new_state, put_file, service, Shared};
use motogp_application::{
    CancelFlag, CatalogError, CatalogInstallError, DownloadError, InstallPhase, InstallProgress,
    StagedDownload,
};

const PAK: &[u8] = b"AAAAAA-pak";

fn prepare(rig: &Rig, id: &str) -> Result<StagedDownload, CatalogInstallError> {
    rig.installer.prepare(
        &rig.catalog,
        &rig.profiles(),
        id,
        &mut |_| {},
        &CancelFlag::new(),
    )
}

/// Copies the staged file into the library fake, which has its own storage.
fn mirror(rig: &Rig, lib: &Shared, staged: &StagedDownload) {
    let content = rig.disk.lock().unwrap().files[staged.path()].clone();
    put_file(lib, &staged.path().to_string_lossy(), &content);
}

fn with_download(size: u64, sha256: &str) -> Rig {
    Rig::new(
        vec![downloadable(
            "rebalanced-ai",
            "https://e.com/a.pak",
            size,
            sha256,
        )],
        PAK,
    )
}

#[test]
fn prepare_stages_the_verified_file_under_its_url_name() {
    let rig = Rig::matching(PAK);
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    assert_eq!(staged.entry_id(), "rebalanced-ai");
    assert_eq!(staged.size(), PAK.len() as u64);
    assert_eq!(
        staged.path().to_string_lossy().replace('\\', "/"),
        "downloads/rebalanced-ai/rebalanced.pak"
    );
    let part = rig.downloader.dests().remove(0);
    assert!(part.to_string_lossy().ends_with("rebalanced.pak.part"));
    assert!(!rig.disk.lock().unwrap().files.contains_key(&part));
}

#[test]
fn the_download_is_capped_at_the_declared_size() {
    let rig = Rig::matching(PAK);
    prepare(&rig, "rebalanced-ai").unwrap();
    let request = rig.downloader.requests().remove(0);
    assert_eq!(request.url, "https://example.com/files/rebalanced.pak");
    assert_eq!(request.max_bytes, PAK.len() as u64);
}

#[test]
fn install_imports_the_staged_file_and_clears_staging() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    mirror(&rig, &lib, &staged);
    let outcome = rig
        .installer
        .install(&service(&lib), &rig.profiles(), staged, None)
        .unwrap();
    assert_eq!(outcome.mods.len(), 1);
    assert_eq!(lib.borrow().index, outcome.mods);
    assert!(!outcome.enabled && outcome.conflict_with.is_none());
    assert!(rig.disk_is_empty());
}

#[test]
fn install_failure_still_clears_staging() {
    // A staged file the importer cannot read: the library refuses it.
    let rig = Rig::new(
        vec![downloadable(
            "rebalanced-ai",
            "https://e.com/notes.txt",
            PAK.len() as u64,
            &fake_hash(PAK),
        )],
        PAK,
    );
    let lib = new_state();
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    let result = rig
        .installer
        .install(&service(&lib), &rig.profiles(), staged, None);
    assert!(matches!(result, Err(CatalogInstallError::Library(_))));
    assert!(rig.disk_is_empty());
}

#[test]
fn install_rechecks_the_session_lock_and_clears_staging() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    mirror(&rig, &lib, &staged);
    rig.session.set_active(true);
    let result = rig
        .installer
        .install(&service(&lib), &rig.profiles(), staged, None);
    assert_eq!(result, Err(CatalogInstallError::SessionActive));
    assert!(lib.borrow().index.is_empty());
    assert!(rig.disk_is_empty());
}

#[test]
fn discard_clears_staging_and_frees_the_entry() {
    let rig = Rig::matching(PAK);
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    rig.installer.discard(staged);
    assert!(rig.disk_is_empty());
    assert!(prepare(&rig, "rebalanced-ai").is_ok());
}

#[test]
fn a_size_mismatch_fails_and_clears_staging() {
    let rig = with_download(99, &fake_hash(PAK));
    assert_eq!(
        prepare(&rig, "rebalanced-ai").unwrap_err(),
        CatalogInstallError::SizeMismatch {
            expected: 99,
            actual: PAK.len() as u64
        }
    );
    assert!(rig.disk_is_empty());
}

#[test]
fn a_hash_mismatch_fails_and_clears_staging() {
    let rig = with_download(PAK.len() as u64, &"0".repeat(64));
    assert_eq!(
        prepare(&rig, "rebalanced-ai").unwrap_err(),
        CatalogInstallError::HashMismatch
    );
    assert!(rig.disk_is_empty());
}

#[test]
fn an_unknown_entry_is_not_found() {
    let rig = Rig::matching(PAK);
    assert_eq!(
        prepare(&rig, "ghost").unwrap_err(),
        CatalogInstallError::EntryNotFound { id: "ghost".into() }
    );
    assert_eq!(rig.downloader.calls(), 0);
}

#[test]
fn an_entry_without_download_is_not_downloadable() {
    let rig = Rig::new(vec![catalog_fakes::raw("page-only")], PAK);
    assert_eq!(
        prepare(&rig, "page-only").unwrap_err(),
        CatalogInstallError::NotDownloadable {
            id: "page-only".into()
        }
    );
    assert_eq!(rig.downloader.calls(), 0);
    assert!(rig.disk_is_empty());
}

#[test]
fn an_active_session_refuses_before_anything_is_downloaded() {
    let rig = Rig::matching(PAK);
    rig.session.set_active(true);
    assert_eq!(
        prepare(&rig, "rebalanced-ai").unwrap_err(),
        CatalogInstallError::SessionActive
    );
    assert_eq!(rig.downloader.calls(), 0);
    assert!(rig.disk_is_empty());
}

#[test]
fn an_unavailable_catalog_is_reported() {
    let rig = Rig::matching(PAK);
    let source = catalog_fakes::FakeSource::failing(
        motogp_application::CatalogSourceError::Network("down".into()),
    );
    let catalog = motogp_application::CatalogService::new(
        source,
        catalog_fakes::FakeCache::default(),
        catalog_fakes::FakeClock::at(catalog_fakes::NOW_MS),
    );
    let result = rig.installer.prepare(
        &catalog,
        &rig.profiles(),
        "rebalanced-ai",
        &mut |_| {},
        &CancelFlag::new(),
    );
    assert!(matches!(
        result,
        Err(CatalogInstallError::Catalog(
            CatalogError::Unavailable { .. }
        ))
    ));
}

#[test]
fn a_second_install_of_the_same_entry_is_refused_until_the_first_ends() {
    let rig = Rig::matching(PAK);
    let first = prepare(&rig, "rebalanced-ai").unwrap();
    assert_eq!(
        prepare(&rig, "rebalanced-ai").unwrap_err(),
        CatalogInstallError::InstallInProgress {
            id: "rebalanced-ai".into()
        }
    );
    drop(first);
    assert!(prepare(&rig, "rebalanced-ai").is_ok());
}

#[test]
fn a_failed_install_frees_the_entry_for_a_retry() {
    let rig = Rig::matching(PAK);
    rig.downloader
        .behave(Behavior::Fail(DownloadError::Network("down".into())));
    assert_eq!(
        prepare(&rig, "rebalanced-ai").unwrap_err(),
        CatalogInstallError::Network("down".into())
    );
    rig.downloader.behave(Behavior::Serve);
    assert!(prepare(&rig, "rebalanced-ai").is_ok());
}

#[test]
fn downloader_errors_are_mapped() {
    let cases = [
        (
            DownloadError::TooLarge { limit: 10 },
            CatalogInstallError::SizeMismatch {
                expected: 10,
                actual: 11,
            },
        ),
        (
            DownloadError::Storage("disk".into()),
            CatalogInstallError::Storage("disk".into()),
        ),
        (DownloadError::Cancelled, CatalogInstallError::Cancelled),
    ];
    for (download_error, expected) in cases {
        let rig = with_download(10, &"a".repeat(64));
        rig.downloader.behave(Behavior::Fail(download_error));
        assert_eq!(prepare(&rig, "rebalanced-ai").unwrap_err(), expected);
        assert!(rig.disk_is_empty());
    }
}

#[test]
fn a_cancelled_download_leaves_nothing_behind() {
    let rig = Rig::matching(PAK);
    rig.downloader.behave(Behavior::CancelMidway);
    let cancel = CancelFlag::new();
    let result = rig.installer.prepare(
        &rig.catalog,
        &rig.profiles(),
        "rebalanced-ai",
        &mut |_| {},
        &cancel,
    );
    assert_eq!(result.unwrap_err(), CatalogInstallError::Cancelled);
    assert!(cancel.is_cancelled());
    assert!(rig.disk_is_empty());
}

#[test]
fn progress_reports_downloading_then_verifying() {
    let rig = Rig::matching(PAK);
    let mut seen: Vec<InstallProgress> = Vec::new();
    rig.installer
        .prepare(
            &rig.catalog,
            &rig.profiles(),
            "rebalanced-ai",
            &mut |p| seen.push(p),
            &CancelFlag::new(),
        )
        .unwrap();
    let total = PAK.len() as u64;
    assert_eq!(
        seen,
        vec![
            InstallProgress {
                phase: InstallPhase::Downloading,
                received: total,
                total
            },
            InstallProgress {
                phase: InstallPhase::Verifying,
                received: total,
                total
            },
        ]
    );
}

#[test]
fn a_url_without_a_file_name_falls_back_to_the_entry_id() {
    let rig = Rig::new(
        vec![downloadable(
            "rebalanced-ai",
            "https://e.com/",
            PAK.len() as u64,
            &fake_hash(PAK),
        )],
        PAK,
    );
    let staged = prepare(&rig, "rebalanced-ai").unwrap();
    assert!(staged
        .path()
        .to_string_lossy()
        .ends_with("rebalanced-ai.zip"));
}

// --- Auto-enable after install (#129) ---

use config_fakes::{FakeProfiles, FixedClock};
use motogp_application::ProfileService;
use motogp_domain::{ModEntry, Profile};
use std::sync::Arc;

/// Profile service over `profiles`, locked by the rig's session like the real one.
fn profile_service_for(
    rig: &Rig,
    profiles: Vec<Profile>,
) -> ProfileService<FakeProfiles, FixedClock> {
    let store = FakeProfiles::default();
    *store.0.borrow_mut() = profiles;
    ProfileService::new(store, FixedClock(1_000)).with_session(Arc::new(rig.session.clone()))
}

fn profile_with(id: &str, mods: &[&str]) -> Profile {
    let mut profile = Profile::new(id, id).unwrap();
    profile.mod_ids = mods.iter().map(|m| m.to_string()).collect();
    profile
}

fn install_enabling(
    rig: &Rig,
    lib: &Shared,
    profiles: &ProfileService<FakeProfiles, FixedClock>,
    target: Option<&str>,
) -> motogp_application::InstallOutcome {
    let staged = prepare(rig, "rebalanced-ai").unwrap();
    mirror(rig, lib, &staged);
    rig.installer
        .install(&service(lib), profiles, staged, target)
        .unwrap()
}

#[test]
fn install_enables_the_mod_in_the_active_profile() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let profiles = profile_service_for(&rig, vec![profile_with("default", &["other"])]);
    let outcome = install_enabling(&rig, &lib, &profiles, Some("default"));
    assert!(outcome.enabled);
    assert_eq!(outcome.conflict_with, None);
    assert_eq!(outcome.mod_id, outcome.mods[0].id);
    let order = profiles.get("default").unwrap().mod_ids;
    assert_eq!(order, ["other".to_string(), outcome.mod_id]);
}

#[test]
fn a_conflicting_mod_stays_installed_but_disabled_and_names_the_blocker() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    // An enabled mod already ships the same file name.
    lib.borrow_mut().index.push(ModEntry {
        id: "existing".into(),
        name: "Existing Pack".into(),
        files: vec!["rebalanced.pak".into()],
        size_bytes: 1,
    });
    let profiles = profile_service_for(&rig, vec![profile_with("default", &["existing"])]);
    let outcome = install_enabling(&rig, &lib, &profiles, Some("default"));
    assert_eq!(lib.borrow().index.len(), 2);
    assert!(!outcome.enabled);
    assert_eq!(outcome.conflict_with.as_deref(), Some("Existing Pack"));
    assert_eq!(profiles.get("default").unwrap().mod_ids, ["existing"]);
}

#[test]
fn without_a_target_profile_the_mod_is_only_installed() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let profiles = profile_service_for(&rig, vec![profile_with("default", &[])]);
    let outcome = install_enabling(&rig, &lib, &profiles, None);
    assert!(!outcome.enabled);
    assert!(profiles.get("default").unwrap().mod_ids.is_empty());
}

#[test]
fn vanilla_never_receives_mods() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let profiles = profile_service_for(&rig, vec![Profile::vanilla()]);
    let outcome = install_enabling(&rig, &lib, &profiles, Some("vanilla"));
    assert!(!outcome.enabled);
    assert!(profiles.get("vanilla").unwrap().mod_ids.is_empty());
}

#[test]
fn an_unknown_target_profile_keeps_the_install_successful() {
    let rig = Rig::matching(PAK);
    let lib = new_state();
    let profiles = profile_service_for(&rig, vec![profile_with("default", &[])]);
    let outcome = install_enabling(&rig, &lib, &profiles, Some("missing"));
    assert!(!outcome.enabled);
    assert_eq!(lib.borrow().index.len(), 1);
}
