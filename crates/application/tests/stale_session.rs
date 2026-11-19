//! Use-case tests of the stale-manifest recovery (issue #23) with in-memory
//! fakes: detection is read-only, forgetting never touches files, and the
//! confirmed cleanup removes only provably-owned files of the recorded folder.
#[path = "support/deploy_fakes.rs"]
mod fakes;

use fakes::*;
use motogp_application::{DeployError, DeploySession};
use motogp_domain::{Deployed, DeployedFile};
use std::path::PathBuf;

fn stale(state: &Shared) {
    stale_manifest(state, &old_mods_dir(), &["000_old_A.pak", "001_old_B.pak"]);
}

// ---------- session() ----------

#[test]
fn session_is_none_for_an_empty_manifest() {
    let state = new_state();
    assert_eq!(
        service(&state).session(&mods_dir()).unwrap(),
        DeploySession::None
    );
}

#[test]
fn session_is_current_when_the_manifest_points_at_the_authorized_dir() {
    let state = new_state();
    stale_manifest(&state, &mods_dir(), &["000_old_A.pak"]);
    assert_eq!(
        service(&state).session(&mods_dir()).unwrap(),
        DeploySession::Current { tracked: 1 }
    );
}

#[test]
fn session_is_stale_with_the_old_folder_and_counts_files_and_pending() {
    let state = new_state();
    stale(&state);
    state
        .borrow_mut()
        .manifest
        .pending
        .push(DeployedFile::new("002_old_C.pak", "lib/old/C"));
    assert_eq!(
        service(&state).session(&mods_dir()).unwrap(),
        DeploySession::Stale {
            mods_dir: old_mods_dir(),
            tracked: 3
        }
    );
}

#[test]
fn session_is_vanished_when_the_recorded_folder_is_gone() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().disk.clear();
    state.borrow_mut().missing_dirs.push(old_mods_dir());
    assert_eq!(
        service(&state).session(&mods_dir()).unwrap(),
        DeploySession::Vanished {
            mods_dir: old_mods_dir()
        }
    );
}

#[test]
fn session_is_read_only() {
    let state = new_state();
    stale(&state);
    let manifest = state.borrow().manifest.clone();
    let disk = state.borrow().disk.clone();

    service(&state).session(&mods_dir()).unwrap();

    let s = state.borrow();
    assert_eq!(s.manifest, manifest);
    assert_eq!(s.disk, disk);
    assert_eq!(s.manifest_saves, 0);
    assert_eq!(s.owns_calls, 0);
    assert!(s.log.is_empty());
}

#[test]
fn session_propagates_an_unreadable_manifest() {
    let state = new_state();
    state.borrow_mut().manifest_fails_to_load = true;
    assert!(matches!(
        service(&state).session(&mods_dir()),
        Err(DeployError::Storage(_))
    ));
}

// ---------- forget_stale_session() ----------

#[test]
fn forget_clears_a_stale_manifest_without_touching_any_file() {
    let state = new_state();
    stale(&state);

    service(&state).forget_stale_session(&mods_dir()).unwrap();

    let s = state.borrow();
    assert_eq!(s.manifest, Deployed::default());
    assert_eq!(s.owns_calls, 0);
    assert!(s.log.is_empty());
    assert_eq!(s.disk.len(), 2);
}

#[test]
fn forget_clears_a_vanished_manifest() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().disk.clear();
    state.borrow_mut().missing_dirs.push(old_mods_dir());

    service(&state).forget_stale_session(&mods_dir()).unwrap();

    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn forget_with_an_empty_manifest_is_a_no_op() {
    let state = new_state();
    service(&state).forget_stale_session(&mods_dir()).unwrap();
    assert_eq!(state.borrow().manifest_saves, 0);
}

#[test]
fn forget_refuses_the_current_session_and_keeps_the_manifest() {
    let state = new_state();
    stale_manifest(&state, &mods_dir(), &["000_old_A.pak"]);
    let manifest = state.borrow().manifest.clone();

    let error = service(&state)
        .forget_stale_session(&mods_dir())
        .unwrap_err();

    assert!(matches!(error, DeployError::SessionNotStale { .. }));
    assert!(error.to_string().contains("use clean instead"));
    assert_eq!(state.borrow().manifest, manifest);
    assert_eq!(state.borrow().manifest_saves, 0);
}

#[test]
fn forget_reports_a_save_failure_and_keeps_the_manifest() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().fail_save_at = Some(0);
    assert!(service(&state).forget_stale_session(&mods_dir()).is_err());
    assert!(!state.borrow().manifest.is_empty());
}

// ---------- clean_stale_session() ----------

#[test]
fn confirmed_clean_removes_owned_files_of_the_old_folder_and_clears_the_manifest() {
    let state = new_state();
    stale(&state);

    let removed = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap();

    assert_eq!(removed, 2);
    assert!(state.borrow().disk.is_empty());
    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn confirmed_clean_leaves_a_foreign_file_untouched() {
    let state = new_state();
    stale(&state);
    put_disk_in(&state, &old_mods_dir(), "meu_mod.pak", b"manual-bytes");

    service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap();

    assert!(disk_has_in(&state, &old_mods_dir(), "meu_mod.pak"));
    assert_eq!(state.borrow().disk.len(), 1);
}

#[test]
fn confirmed_clean_keeps_a_modified_file_and_untracks_it() {
    let state = new_state();
    stale(&state);
    put_disk_in(&state, &old_mods_dir(), "000_old_A.pak", b"edited");

    let removed = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap();

    assert_eq!(removed, 1);
    assert_eq!(
        state.borrow().disk[&(old_mods_dir(), "000_old_A.pak".to_string())],
        b"edited".to_vec()
    );
    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn clean_with_a_wrong_confirmation_is_refused_and_removes_nothing() {
    let state = new_state();
    stale(&state);
    let manifest = state.borrow().manifest.clone();

    let error = service(&state)
        .clean_stale_session(&mods_dir(), &PathBuf::from("other/~mods"))
        .unwrap_err();

    assert!(matches!(error, DeployError::UnauthorizedDirectory { .. }));
    assert_eq!(state.borrow().disk.len(), 2);
    assert!(state.borrow().log.is_empty());
    assert_eq!(state.borrow().manifest, manifest);
}

#[test]
fn clean_refuses_a_recorded_folder_that_is_not_named_mods() {
    let state = new_state();
    let dir = PathBuf::from("oldgame/Documents");
    stale_manifest(&state, &dir, &["000_old_A.pak"]);

    let error = service(&state)
        .clean_stale_session(&mods_dir(), &dir)
        .unwrap_err();

    assert!(matches!(error, DeployError::UnauthorizedDirectory { .. }));
    assert!(disk_has_in(&state, &dir, "000_old_A.pak"));
    assert!(!state.borrow().manifest.is_empty());
}

#[test]
fn clean_rejects_tampered_names_before_removing_anything() {
    let state = new_state();
    stale(&state);
    state
        .borrow_mut()
        .manifest
        .files
        .push(DeployedFile::new("../evil.pak", "lib/old/evil"));

    let error = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap_err();

    assert!(matches!(error, DeployError::InvalidPath { .. }));
    assert_eq!(state.borrow().disk.len(), 2);
    assert!(state.borrow().log.is_empty());
}

#[test]
fn clean_rejects_an_empty_recorded_directory() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().manifest.mods_dir = PathBuf::new();
    assert!(matches!(
        service(&state).clean_stale_session(&mods_dir(), &old_mods_dir()),
        Err(DeployError::InvalidPath { .. })
    ));
}

#[test]
fn clean_with_an_empty_manifest_is_a_no_op() {
    let state = new_state();
    assert_eq!(
        service(&state)
            .clean_stale_session(&mods_dir(), &old_mods_dir())
            .unwrap(),
        0
    );
    assert_eq!(state.borrow().manifest_saves, 0);
}

#[test]
fn clean_of_a_vanished_folder_clears_the_manifest() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().disk.clear();
    state.borrow_mut().missing_dirs.push(old_mods_dir());

    let removed = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap();

    assert_eq!(removed, 0);
    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn clean_of_the_current_session_behaves_like_the_normal_clean() {
    let state = new_state();
    stale_manifest(&state, &mods_dir(), &["000_old_A.pak"]);

    let removed = service(&state)
        .clean_stale_session(&mods_dir(), &PathBuf::from("anything"))
        .unwrap();

    assert_eq!(removed, 1);
    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn partial_failure_keeps_the_entry_against_the_old_folder_for_a_retry() {
    let state = new_state();
    stale(&state);
    state.borrow_mut().fail_remove = vec!["001_old_B.pak".into()];

    let error = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap_err();

    assert!(matches!(
        error,
        DeployError::CleanIncomplete {
            removed: 1,
            remaining: 1,
            ..
        }
    ));
    let manifest = state.borrow().manifest.clone();
    assert_eq!(manifest.mods_dir, old_mods_dir());
    assert_eq!(manifest_names(&manifest.files), vec!["001_old_B.pak"]);

    // Retry once the failure is gone.
    state.borrow_mut().fail_remove.clear();
    let removed = service(&state)
        .clean_stale_session(&mods_dir(), &old_mods_dir())
        .unwrap();
    assert_eq!(removed, 1);
    assert_eq!(state.borrow().manifest, Deployed::default());
}

// ---------- unchanged behaviour and recovery ----------

#[test]
fn normal_clean_and_deploy_still_refuse_a_stale_manifest() {
    let state = new_state();
    stale(&state);
    assert!(matches!(
        service(&state).clean(&mods_dir()),
        Err(DeployError::UnauthorizedDirectory { .. })
    ));
    assert!(matches!(
        service(&state).deploy(&mods_dir(), &[mod_of(ID_A, &["A.pak"])]),
        Err(DeployError::UnauthorizedDirectory { .. })
    ));
}

#[test]
fn deploy_works_again_after_forgetting_the_old_session() {
    let state = new_state();
    stale(&state);
    service(&state).forget_stale_session(&mods_dir()).unwrap();
    let mods = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);

    let names = service(&state).deploy(&mods_dir(), &mods).unwrap();

    assert_eq!(names, vec!["000_aaaaaaaaaaaa_A.pak"]);
    // The old folder's files were never touched.
    assert!(disk_has_in(&state, &old_mods_dir(), "000_old_A.pak"));
}
