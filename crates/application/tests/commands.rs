//! Use-case tests for the command-facing application services (issue #8):
//! game settings, active profile, batch import, mod removal and profile
//! resolution. In-memory fakes only.
#[path = "support/config_fakes.rs"]
mod config_fakes;
#[path = "support/fakes.rs"]
mod fakes;

use config_fakes::*;
use motogp_application::{import_mods, remove_mod, resolve_profile_mods, GameError, LibraryError};
use motogp_application::{ProfileServiceError, RemoveModError};
use motogp_domain::{Config, ModEntry, Profile, ProfileError, VANILLA_ID};
use std::path::{Path, PathBuf};

fn entry(id: &str, file: &str) -> ModEntry {
    ModEntry {
        id: id.into(),
        name: file.into(),
        files: vec![format!("{file}.pak")],
        size_bytes: 1,
    }
}

#[test]
fn resolve_profile_mods_keeps_order_and_skips_missing() {
    let mut profile = Profile::new("p1", "Race").unwrap();
    profile.mod_ids = vec!["bbbbbbbbbbbb".into(), "gone00000000".into(), "aaaaaaaaaaaa".into()];
    let library = vec![entry("aaaaaaaaaaaa", "A"), entry("bbbbbbbbbbbb", "B")];

    let resolved = resolve_profile_mods(&profile, &library, Path::new("lib"));

    let ids: Vec<&str> = resolved.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["bbbbbbbbbbbb", "aaaaaaaaaaaa"]);
    assert_eq!(resolved[0].source_dir, Path::new("lib").join("bbbbbbbbbbbb"));
    assert_eq!(resolved[0].files, vec!["B.pak"]);
}

#[test]
fn set_game_dir_inspects_before_saving() {
    let config = FakeConfig::default();
    let settings = settings(&config, FakeGameFiles::default());

    let error = settings.set_game_dir(Path::new("not-a-game")).unwrap_err();

    assert!(matches!(error, GameError::NotUnrealGame { .. }));
    assert_eq!(config.0.borrow().saves, 0, "nothing saved for an invalid folder");
    assert_eq!(config.0.borrow().config.game_dir, None);
}

#[test]
fn set_game_dir_saves_the_inspected_game() {
    let config = FakeConfig::default();
    let files = FakeGameFiles {
        valid_roots: vec![valid_root()],
        ..Default::default()
    };
    let settings = settings(&config, files);

    let game = settings.set_game_dir(&valid_root()).unwrap();

    assert_eq!(game.app_id, Some(123));
    assert_eq!(config.0.borrow().config.game_dir, Some(valid_root()));
}

#[test]
fn set_game_dir_keeps_the_active_profile() {
    let config = FakeConfig::default();
    config.0.borrow_mut().config.active_profile = Some("p1".into());
    let files = FakeGameFiles {
        valid_roots: vec![valid_root()],
        ..Default::default()
    };

    settings(&config, files).set_game_dir(&valid_root()).unwrap();

    assert_eq!(config.0.borrow().config.active_profile.as_deref(), Some("p1"));
}

#[test]
fn get_game_is_none_only_when_unconfigured() {
    let config = FakeConfig::default();
    let settings = settings(&config, FakeGameFiles::default());
    assert_eq!(settings.get_game(), Ok(None));

    config.0.borrow_mut().config.game_dir = Some(PathBuf::from("moved-away"));
    let error = settings.get_game().unwrap_err();
    assert!(matches!(error, GameError::NotUnrealGame { .. }));
}

#[test]
fn get_game_surfaces_inspection_errors_of_a_configured_path() {
    let config = FakeConfig::default();
    config.0.borrow_mut().config.game_dir = Some(valid_root());
    let files = FakeGameFiles {
        valid_roots: vec![valid_root()],
        manifests_fail: true,
    };

    let error = settings(&config, files).get_game().unwrap_err();

    assert!(matches!(error, GameError::Storage(_)));
}

#[test]
fn get_config_returns_the_stored_configuration() {
    let config = FakeConfig::default();
    config.0.borrow_mut().config = Config {
        game_dir: Some(PathBuf::from("g")),
        active_profile: Some("p1".into()),
    };
    let settings = settings(&config, FakeGameFiles::default());

    assert_eq!(settings.config().unwrap(), config.0.borrow().config);
}

#[test]
fn set_active_profile_validates_the_profile() {
    let profiles = FakeProfiles::default();
    let config = FakeConfig::default();
    let service = profile_service(&profiles);
    let created = service.create("Race").unwrap();

    let error = service.set_active("missing", &config).unwrap_err();
    assert_eq!(error, ProfileServiceError::Profile(ProfileError::NotFound));
    assert_eq!(config.0.borrow().saves, 0);

    service.set_active(&created.id, &config).unwrap();
    assert_eq!(config.0.borrow().config.active_profile, Some(created.id));
    service.set_active(VANILLA_ID, &config).unwrap();
    assert_eq!(config.0.borrow().config.active_profile.as_deref(), Some(VANILLA_ID));
}

#[test]
fn deleting_active_profile_selects_vanilla() {
    let profiles = FakeProfiles::default();
    let config = FakeConfig::default();
    let service = profile_service(&profiles);
    let created = service.create("Race").unwrap();
    service.set_active(&created.id, &config).unwrap();

    service.delete(&created.id, &config).unwrap();

    assert_eq!(config.0.borrow().config.active_profile.as_deref(), Some(VANILLA_ID));
    assert!(profiles.0.borrow().iter().all(|p| p.id != created.id));
    assert_eq!(
        service.delete(VANILLA_ID, &config).unwrap_err(),
        ProfileServiceError::Profile(ProfileError::VanillaProtected)
    );
}

#[test]
fn remove_mod_clears_profile_references() {
    let state = fakes::new_state();
    fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    let mods = fakes::service(&state);
    let id = mods.import(Path::new("src/A.pak")).unwrap()[0].id.clone();
    let profiles = FakeProfiles::default();
    let service = profile_service(&profiles);
    let first = service.create("One").unwrap();
    let second = service.create("Two").unwrap();
    service.set_mods(&first.id, vec![id.clone(), "other0000000".into()]).unwrap();
    service.set_mods(&second.id, vec![id.clone()]).unwrap();

    remove_mod(&mods, &service, &id).unwrap();

    assert!(state.borrow().index.is_empty());
    for profile in profiles.0.borrow().iter() {
        assert!(!profile.mod_ids.contains(&id), "{} still references the mod", profile.name);
    }
    let first = service.get(&first.id).unwrap();
    assert_eq!(first.mod_ids, vec!["other0000000"], "other mods are kept");
}

#[test]
fn remove_mod_reports_library_failures_and_keeps_references() {
    let state = fakes::new_state();
    fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    let mods = fakes::service(&state);
    let id = mods.import(Path::new("src/A.pak")).unwrap()[0].id.clone();
    let profiles = FakeProfiles::default();
    let service = profile_service(&profiles);
    let profile = service.create("One").unwrap();
    service.set_mods(&profile.id, vec![id.clone()]).unwrap();
    state.borrow_mut().fail_remove_dir = true;

    let error = remove_mod(&mods, &service, &id).unwrap_err();

    assert!(matches!(error, RemoveModError::Library(LibraryError::Storage(_))));
    assert_eq!(service.get(&profile.id).unwrap().mod_ids, vec![id], "references stay");
}

#[test]
fn remove_unknown_mod_reports_not_found() {
    let state = fakes::new_state();
    let mods = fakes::service(&state);
    let service = profile_service(&FakeProfiles::default());

    let error = remove_mod(&mods, &service, "aaaaaaaaaaaa").unwrap_err();

    assert!(matches!(error, RemoveModError::Library(LibraryError::NotFound { .. })));
}

#[test]
fn batch_import_returns_every_entry_in_order() {
    let state = fakes::new_state();
    fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    fakes::put_file(&state, "src/B.pak", b"BBBBBB-pak");
    let mods = fakes::service(&state);

    let entries = import_mods(&mods, &[PathBuf::from("src/A.pak"), PathBuf::from("src/B.pak")]).unwrap();

    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B"]);
    assert_eq!(state.borrow().index.len(), 2);
}

#[test]
fn batch_import_stops_at_first_error_and_keeps_completed_imports() {
    let state = fakes::new_state();
    fakes::put_file(&state, "src/A.pak", b"AAAAAA-pak");
    fakes::put_file(&state, "src/C.pak", b"CCCCCC-pak");
    let mods = fakes::service(&state);
    let paths = [
        PathBuf::from("src/A.pak"),
        PathBuf::from("src/notes.txt"),
        PathBuf::from("src/C.pak"),
    ];

    let error = import_mods(&mods, &paths).unwrap_err();

    assert_eq!(error.failed, PathBuf::from("src/notes.txt"));
    assert!(matches!(error.error, LibraryError::UnsupportedFormat { .. }));
    assert_eq!(error.completed.len(), 1);
    assert_eq!(error.completed[0].name, "A");
    let index = state.borrow().index.clone();
    assert_eq!(index.len(), 1, "A stays in the library, C was never imported");
    assert_eq!(index[0].name, "A");
}
