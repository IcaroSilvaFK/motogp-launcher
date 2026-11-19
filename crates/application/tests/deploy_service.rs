//! Use-case tests of deploy/clean with in-memory fakes (no real fs).
#[path = "support/deploy_fakes.rs"]
mod fakes;

use fakes::*;
use motogp_application::DeployError;
use motogp_domain::Deployed;
use std::path::PathBuf;

fn deploy_a_and_b(state: &Shared) -> Vec<String> {
    let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
    put_sources(state, &mods);
    service(state).deploy(&mods_dir(), &mods).unwrap()
}

#[test]
fn deploy_writes_files_in_profile_order_with_prefix() {
    let state = new_state();

    let names = deploy_a_and_b(&state);

    assert_eq!(
        names,
        vec!["000_bbbbbbbbbbbb_B.pak", "001_aaaaaaaaaaaa_A.pak"]
    );
    assert_eq!(disk_names(&state).len(), 2);
    assert!(disk_has(&state, "000_bbbbbbbbbbbb_B.pak"));
    assert!(disk_has(&state, "001_aaaaaaaaaaaa_A.pak"));
    let manifest = state.borrow().manifest.clone();
    assert_eq!(manifest.mods_dir, mods_dir());
    assert_eq!(manifest.files, names);
    assert!(manifest.pending.is_empty());
}

#[test]
fn files_of_the_same_mod_share_the_position_index() {
    let state = new_state();
    let mods = [mod_of(ID_A, &["A_P.pak", "A_P.utoc", "A_P.ucas"])];
    put_sources(&state, &mods);

    let names = service(&state).deploy(&mods_dir(), &mods).unwrap();

    assert!(names.iter().all(|n| n.starts_with("000_aaaaaaaaaaaa_A_P.")));
    assert_eq!(names.len(), 3);
}

#[test]
fn every_deployed_file_has_a_persisted_record_before_it_exists() {
    let state = new_state();
    let mods = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);
    // Crash right before the materialization: the intent must already be saved.
    state.borrow_mut().crash_before_materialize = Some(0);

    assert!(service(&state).deploy(&mods_dir(), &mods).is_err());

    let manifest = state.borrow().manifest.clone();
    assert_eq!(manifest.pending, vec!["000_aaaaaaaaaaaa_A.pak"]);
}

#[test]
fn deploy_cleans_previous_session_first() {
    let state = new_state();
    let mods_a = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods_a);
    service(&state).deploy(&mods_dir(), &mods_a).unwrap();
    state.borrow_mut().log.clear();

    let mods_b = [mod_of(ID_B, &["B.pak"])];
    put_sources(&state, &mods_b);
    let names = service(&state).deploy(&mods_dir(), &mods_b).unwrap();

    let log = state.borrow().log.clone();
    assert_eq!(
        log,
        vec![
            "remove 000_aaaaaaaaaaaa_A.pak",
            "materialize 000_bbbbbbbbbbbb_B.pak"
        ]
    );
    assert_eq!(state.borrow().manifest.files, names);
    assert_eq!(disk_names(&state), vec!["000_bbbbbbbbbbbb_B.pak"]);
}

#[test]
fn deploy_does_not_proceed_when_cleaning_the_previous_session_fails() {
    let state = new_state();
    let mods_a = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods_a);
    service(&state).deploy(&mods_dir(), &mods_a).unwrap();
    state.borrow_mut().fail_remove = vec!["000_aaaaaaaaaaaa_A.pak".into()];

    let mods_b = [mod_of(ID_B, &["B.pak"])];
    put_sources(&state, &mods_b);
    let error = service(&state).deploy(&mods_dir(), &mods_b).unwrap_err();

    assert!(matches!(error, DeployError::CleanIncomplete { .. }));
    assert_eq!(disk_names(&state), vec!["000_aaaaaaaaaaaa_A.pak"]);
    assert_eq!(state.borrow().materialize_calls, 1);
}

#[test]
fn deploy_does_not_proceed_when_the_manifest_cannot_be_read() {
    let state = new_state();
    state.borrow_mut().manifest_fails_to_load = true;
    let mods = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);

    let error = service(&state).deploy(&mods_dir(), &mods).unwrap_err();

    assert!(matches!(error, DeployError::Storage(_)));
    assert_eq!(state.borrow().materialize_calls, 0);
}

#[test]
fn deploy_empty_profile_leaves_dir_untouched() {
    let state = new_state();

    let names = service(&state).deploy(&mods_dir(), &[]).unwrap();

    assert!(names.is_empty());
    assert!(state.borrow().dirs.is_empty());
    assert!(state.borrow().disk.is_empty());
    assert_eq!(state.borrow().manifest_saves, 0);
}

#[test]
fn deploy_mods_without_files_creates_nothing() {
    let state = new_state();

    service(&state)
        .deploy(&mods_dir(), &[mod_of(ID_A, &[])])
        .unwrap();

    assert!(state.borrow().dirs.is_empty());
}

#[test]
fn deploy_refuses_a_destination_that_already_exists_and_leaves_it_alone() {
    let state = new_state();
    put_disk(&state, "000_aaaaaaaaaaaa_A.pak", b"manual");
    let mods = [mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);

    let error = service(&state).deploy(&mods_dir(), &mods).unwrap_err();

    assert!(error.to_string().contains("already exists"), "{error}");
    assert!(state.borrow().manifest.is_empty());
    assert_eq!(state.borrow().materialize_calls, 0);
    // A later cleanup must never touch the foreign file.
    service(&state).clean().unwrap();
    assert!(disk_has(&state, "000_aaaaaaaaaaaa_A.pak"));
}

#[test]
fn deploy_refuses_before_writing_anything_when_a_later_destination_collides() {
    let state = new_state();
    put_disk(&state, "001_aaaaaaaaaaaa_A.pak", b"manual");
    let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);

    assert!(service(&state).deploy(&mods_dir(), &mods).is_err());

    assert_eq!(disk_names(&state), vec!["001_aaaaaaaaaaaa_A.pak"]);
    assert!(state.borrow().manifest.is_empty());
}

#[test]
fn deploy_rejects_unsafe_names_and_ids_before_writing() {
    let state = new_state();
    for bad in [
        mod_of(ID_A, &["..\\evil.pak"]),
        mod_of(ID_A, &["sub/evil.pak"]),
        mod_of("../up", &["A.pak"]),
    ] {
        let error = service(&state).deploy(&mods_dir(), &[bad]).unwrap_err();
        assert!(matches!(error, DeployError::InvalidPath { .. }), "{error}");
    }
    assert_eq!(state.borrow().materialize_calls, 0);
    assert!(state.borrow().dirs.is_empty());
}

#[test]
fn interrupted_deploy_remains_recoverable() {
    // Interruptions between recording the intent and materializing, and between
    // materializing and recording the result, for both files of the profile.
    for (before, after) in [
        (Some(0), None),
        (Some(1), None),
        (None, Some(0)),
        (None, Some(1)),
    ] {
        let state = new_state();
        put_disk(&state, "meu_mod.pak", b"manual");
        let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
        put_sources(&state, &mods);
        {
            let mut s = state.borrow_mut();
            s.crash_before_materialize = before;
            s.crash_after_materialize = after;
        }

        assert!(service(&state).deploy(&mods_dir(), &mods).is_err());

        // No deployed file is left without a recoverable record.
        let manifest = state.borrow().manifest.clone();
        let tracked: Vec<&str> = manifest.tracked().collect();
        for name in disk_names(&state) {
            if name != "meu_mod.pak" {
                assert!(tracked.contains(&name.as_str()), "{name} is untracked");
            }
        }

        // The next launch recovers from the manifest and journal.
        state.borrow_mut().dead = false;
        service(&state).clean().unwrap();

        assert_eq!(
            disk_names(&state),
            vec!["meu_mod.pak"],
            "{before:?}/{after:?}"
        );
        assert!(state.borrow().manifest.is_empty());
    }
}

#[test]
fn a_failed_materialization_rolls_the_session_back() {
    let state = new_state();
    let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);
    state.borrow_mut().fail_materialize = Some(1);

    let error = service(&state).deploy(&mods_dir(), &mods).unwrap_err();

    assert!(error.to_string().contains("disk full"), "{error}");
    assert!(state.borrow().disk.is_empty());
    assert!(state.borrow().manifest.is_empty());
}

#[test]
fn a_rollback_that_cannot_finish_keeps_the_files_tracked() {
    let state = new_state();
    let mods = [mod_of(ID_B, &["B.pak"]), mod_of(ID_A, &["A.pak"])];
    put_sources(&state, &mods);
    {
        let mut s = state.borrow_mut();
        s.fail_materialize = Some(1);
        s.fail_remove = vec!["000_bbbbbbbbbbbb_B.pak".into()];
    }

    assert!(service(&state).deploy(&mods_dir(), &mods).is_err());

    let manifest = state.borrow().manifest.clone();
    assert!(manifest.tracked().any(|n| n == "000_bbbbbbbbbbbb_B.pak"));
}

#[test]
fn clean_removes_only_registered_files_and_reports_the_count() {
    let state = new_state();
    deploy_a_and_b(&state);
    put_disk(&state, "meu_mod.pak", b"manual");

    let removed = service(&state).clean().unwrap();

    assert_eq!(removed, 2);
    assert_eq!(disk_names(&state), vec!["meu_mod.pak"]);
    assert_eq!(state.borrow().manifest, Deployed::default());
}

#[test]
fn clean_without_a_session_does_nothing() {
    let state = new_state();

    assert_eq!(service(&state).clean().unwrap(), 0);
    assert_eq!(state.borrow().manifest_saves, 0);
}

#[test]
fn clean_tolerates_an_already_missing_file_and_counts_only_removed_ones() {
    let state = new_state();
    deploy_a_and_b(&state);
    state
        .borrow_mut()
        .disk
        .remove(&(mods_dir(), "000_bbbbbbbbbbbb_B.pak".to_string()));

    let removed = service(&state).clean().unwrap();

    assert_eq!(removed, 1);
    assert!(state.borrow().manifest.is_empty());
}

#[test]
fn failed_clean_keeps_pending_entries() {
    let state = new_state();
    deploy_a_and_b(&state);
    state.borrow_mut().fail_remove = vec!["001_aaaaaaaaaaaa_A.pak".into()];

    let error = service(&state).clean().unwrap_err();

    assert!(matches!(
        error,
        DeployError::CleanIncomplete {
            removed: 1,
            remaining: 1,
            ..
        }
    ));
    let manifest = state.borrow().manifest.clone();
    assert_eq!(manifest.files, vec!["001_aaaaaaaaaaaa_A.pak"]);
    assert_eq!(manifest.mods_dir, mods_dir());
    // A retry after the failure is gone finishes the job.
    state.borrow_mut().fail_remove.clear();
    assert_eq!(service(&state).clean().unwrap(), 1);
    assert!(state.borrow().manifest.is_empty());
}

#[test]
fn invalid_manifest_path_is_rejected() {
    for bad in ["../evil.pak", "..\\evil.pak", "a/b.pak", "C:\\x.pak", ""] {
        let state = new_state();
        put_disk(&state, "000_aaaaaaaaaaaa_A.pak", b"ours");
        state.borrow_mut().disk.insert(
            (PathBuf::from("game"), "evil.pak".to_string()),
            b"external".to_vec(),
        );
        state.borrow_mut().manifest = Deployed {
            mods_dir: mods_dir(),
            files: vec!["000_aaaaaaaaaaaa_A.pak".into(), bad.into()],
            pending: vec![],
        };

        let error = service(&state).clean().unwrap_err();

        assert!(matches!(error, DeployError::InvalidPath { .. }), "{bad:?}");
        // Validation happens before any removal, and the manifest is kept.
        assert!(state.borrow().log.is_empty());
        assert_eq!(state.borrow().disk.len(), 2);
        assert_eq!(state.borrow().manifest.files.len(), 2);
    }
}

#[test]
fn a_manifest_with_files_but_no_directory_is_rejected() {
    let state = new_state();
    state.borrow_mut().manifest = Deployed {
        mods_dir: PathBuf::new(),
        files: vec!["a.pak".into()],
        pending: vec![],
    };

    let error = service(&state).clean().unwrap_err();

    assert!(matches!(error, DeployError::InvalidPath { .. }));
}

#[test]
fn deploy_error_messages_are_descriptive() {
    let cases = [
        (
            DeployError::AlreadyExists { name: "a".into() },
            "already exists",
        ),
        (
            DeployError::InvalidPath { path: "..".into() },
            "invalid path",
        ),
        (io_error("boom"), "boom"),
        (
            DeployError::CleanIncomplete {
                removed: 1,
                remaining: 2,
                detail: "denied".into(),
            },
            "2 file(s) remain",
        ),
    ];
    for (error, text) in cases {
        assert!(error.to_string().contains(text), "{error}");
    }
}
