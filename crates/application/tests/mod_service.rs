//! Use-case tests with in-memory fakes (no fs, ZIP or hashing).
#[path = "support/fakes.rs"]
mod fakes;

use fakes::*;
use motogp_application::{LibraryError, LibraryFiles};
use motogp_domain::GroupingError;
use std::path::Path;

const PAK_A: &[u8] = b"AAAAAA-pak";
const PAK_B: &[u8] = b"BBBBBB-pak";

#[test]
fn import_single_pak_registers_the_entry() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);

    let entries = service(&state).import(Path::new("src/A.pak")).unwrap();

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.id, id_of(PAK_A));
    assert_eq!(entry.name, "A");
    assert_eq!(entry.files, vec!["A.pak".to_string()]);
    assert_eq!(entry.size_bytes, PAK_A.len() as u64);
    assert_eq!(state.borrow().index, entries);
    assert_eq!(library_files_of(&state, &entry.id), vec!["A.pak"]);
    assert!(staging_leftovers(&state).is_empty());
    assert_eq!(state.borrow().discarded.len(), 1);
}

#[test]
fn import_picks_only_the_siblings_of_the_chosen_mod() {
    let state = new_state();
    put_file(&state, "src/A_P.pak", PAK_A);
    put_file(&state, "src/A_P.utoc", b"utoc");
    put_file(&state, "src/a_p.UCAS", b"ucas!");
    put_file(&state, "src/B.pak", PAK_B);
    put_file(&state, "src/A_P.txt", b"notes");

    let entries = service(&state).import(Path::new("src/A_P.pak")).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "A");
    let mut files = entries[0].files.clone();
    files.sort();
    assert_eq!(files, vec!["A_P.pak", "A_P.utoc", "a_p.UCAS"]);
    assert_eq!(entries[0].size_bytes, (PAK_A.len() + 4 + 5) as u64);
    assert_eq!(state.borrow().index.len(), 1);
}

#[test]
fn import_utoc_without_pak_sibling_errors() {
    let state = new_state();
    put_file(&state, "src/A.utoc", b"utoc");

    let error = service(&state).import(Path::new("src/A.utoc")).unwrap_err();

    assert_eq!(
        error,
        LibraryError::Grouping(GroupingError::MissingPak { group: "A".into() })
    );
    assert!(state.borrow().index.is_empty());
    assert!(staging_leftovers(&state).is_empty());
}

#[test]
fn import_zip_returns_every_valid_group() {
    let state = new_state();
    put_archive(
        &state,
        "mods.zip",
        &[("A.pak", PAK_A), ("B.pak", PAK_B), ("B.utoc", b"utoc")],
    );

    let entries = service(&state).import(Path::new("mods.zip")).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "A");
    assert_eq!(entries[1].name, "B");
    assert_eq!(entries[1].files, vec!["B.pak", "B.utoc"]);
    assert_eq!(state.borrow().index.len(), 2);
    assert!(staging_leftovers(&state).is_empty());
}

#[test]
fn zip_without_pak_errors() {
    let state = new_state();
    put_archive(&state, "mods.zip", &[("readme.txt", b"hi")]);

    let error = service(&state).import(Path::new("mods.zip")).unwrap_err();

    assert_eq!(error, LibraryError::NoPakFound);
    assert!(state.borrow().index.is_empty());
    assert!(staging_leftovers(&state).is_empty());
}

#[test]
fn zip_with_a_group_missing_its_pak_errors() {
    let state = new_state();
    put_archive(&state, "mods.zip", &[("A.pak", PAK_A), ("B.utoc", b"u")]);

    let error = service(&state).import(Path::new("mods.zip")).unwrap_err();

    assert_eq!(
        error,
        LibraryError::Grouping(GroupingError::MissingPak { group: "B".into() })
    );
    assert!(state.borrow().index.is_empty());
}

#[test]
fn reimport_same_pak_does_not_duplicate() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    let service = service(&state);

    let first = service.import(Path::new("src/A.pak")).unwrap();
    let second = service.import(Path::new("src/A.pak")).unwrap();

    assert_eq!(first, second);
    assert_eq!(state.borrow().index.len(), 1);
    assert_eq!(library_files_of(&state, &first[0].id), vec!["A.pak"]);
}

#[test]
fn same_content_under_another_name_is_the_same_mod() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    put_file(&state, "other/Renamed.pak", PAK_A);
    let service = service(&state);

    let first = service.import(Path::new("src/A.pak")).unwrap();
    let second = service.import(Path::new("other/Renamed.pak")).unwrap();

    assert_eq!(first, second);
    assert_eq!(state.borrow().index.len(), 1);
}

#[test]
fn unsupported_extension_is_rejected() {
    let state = new_state();
    put_file(&state, "src/mod.rar", b"rar");

    for source in ["src/mod.rar", "src/noextension"] {
        let error = service(&state).import(Path::new(source)).unwrap_err();
        assert!(
            matches!(error, LibraryError::UnsupportedFormat { .. }),
            "{error:?}"
        );
        assert!(error.to_string().contains("unsupported format"));
    }
    assert!(state.borrow().index.is_empty());
    assert_eq!(state.borrow().staging_counter, 0, "nothing was staged");
}

#[test]
fn zip_extension_is_case_insensitive() {
    let state = new_state();
    put_archive(&state, "Mods.ZIP", &[("A.pak", PAK_A)]);
    assert_eq!(
        service(&state).import(Path::new("Mods.ZIP")).unwrap().len(),
        1
    );
}

#[test]
fn failed_import_does_not_publish_index_entry() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    state.borrow_mut().fail_copy = true;

    let error = service(&state).import(Path::new("src/A.pak")).unwrap_err();

    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
    assert!(state.borrow().index.is_empty());
    assert!(library_files_of(&state, &id_of(PAK_A)).is_empty());
    assert!(staging_leftovers(&state).is_empty());
    assert_eq!(state.borrow().discarded.len(), 1);
}

#[test]
fn index_failure_rolls_back_the_published_folder() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    state.borrow_mut().fail_index_add = true;

    let error = service(&state).import(Path::new("src/A.pak")).unwrap_err();

    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
    assert!(state.borrow().index.is_empty());
    assert!(library_files_of(&state, &id_of(PAK_A)).is_empty());
}

#[test]
fn missing_archive_is_a_storage_error() {
    let state = new_state();
    let error = service(&state).import(Path::new("nope.zip")).unwrap_err();
    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
}

#[test]
fn list_returns_the_indexed_mods() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    let service = service(&state);
    assert!(service.list().unwrap().is_empty());

    let imported = service.import(Path::new("src/A.pak")).unwrap();

    assert_eq!(service.list().unwrap(), imported);
}

#[test]
fn remove_deletes_the_folder_and_the_index_entry() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    let service = service(&state);
    let id = service.import(Path::new("src/A.pak")).unwrap()[0]
        .id
        .clone();

    service.remove(&id).unwrap();

    assert!(state.borrow().index.is_empty());
    assert!(library_files_of(&state, &id).is_empty());
}

#[test]
fn remove_unknown_id_is_not_found() {
    let state = new_state();
    let error = service(&state).remove("0123456789ab").unwrap_err();
    assert_eq!(
        error,
        LibraryError::NotFound {
            id: "0123456789ab".into()
        }
    );
}

#[test]
fn remove_rejects_ids_that_are_not_twelve_lowercase_hex() {
    let state = new_state();
    for id in ["", "..", "../library", "ABCDEF012345", "short"] {
        let error = service(&state).remove(id).unwrap_err();
        assert_eq!(error, LibraryError::InvalidId { id: id.into() });
    }
}

#[test]
fn zip_import_with_a_failing_second_group_leaves_the_index_unchanged() {
    let state = new_state();
    put_file(&state, "src/C.pak", b"CCCCCC-pak");
    let service = service(&state);
    let existing = service.import(Path::new("src/C.pak")).unwrap();
    put_archive(&state, "mods.zip", &[("A.pak", PAK_A), ("B.pak", PAK_B)]);
    state.borrow_mut().fail_publish_id = Some(id_of(PAK_B));

    let error = service.import(Path::new("mods.zip")).unwrap_err();

    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
    assert_eq!(state.borrow().index, existing, "no new entries");
    assert!(library_files_of(&state, &id_of(PAK_A)).is_empty());
    assert!(library_files_of(&state, &id_of(PAK_B)).is_empty());
    assert_eq!(
        library_files_of(&state, &existing[0].id),
        vec!["C.pak"],
        "preexisting mods are preserved"
    );
    assert!(staging_leftovers(&state).is_empty());
}

#[test]
fn zip_import_index_failure_removes_every_folder_published_by_the_import() {
    let state = new_state();
    put_archive(&state, "mods.zip", &[("A.pak", PAK_A), ("B.pak", PAK_B)]);
    state.borrow_mut().fail_index_add = true;

    assert!(service(&state).import(Path::new("mods.zip")).is_err());

    assert!(state.borrow().index.is_empty());
    assert!(library_files_of(&state, &id_of(PAK_A)).is_empty());
    assert!(library_files_of(&state, &id_of(PAK_B)).is_empty());
}

#[test]
fn failed_folder_deletion_keeps_the_entry_and_a_retry_succeeds() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    let service = service(&state);
    let id = service.import(Path::new("src/A.pak")).unwrap()[0]
        .id
        .clone();
    state.borrow_mut().fail_remove_dir = true;

    let error = service.remove(&id).unwrap_err();

    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
    assert_eq!(state.borrow().index.len(), 1, "the entry is kept");
    assert_eq!(library_files_of(&state, &id), vec!["A.pak"]);

    state.borrow_mut().fail_remove_dir = false;
    service.remove(&id).unwrap();
    assert!(state.borrow().index.is_empty());
    assert!(library_files_of(&state, &id).is_empty());
}

#[test]
fn remove_retries_when_the_folder_is_gone_but_the_entry_remains() {
    let state = new_state();
    put_file(&state, "src/A.pak", PAK_A);
    let service = service(&state);
    let id = service.import(Path::new("src/A.pak")).unwrap()[0]
        .id
        .clone();
    FakeFiles(state.clone()).remove_mod_dir(&id).unwrap();

    service.remove(&id).unwrap();

    assert!(state.borrow().index.is_empty());
}
