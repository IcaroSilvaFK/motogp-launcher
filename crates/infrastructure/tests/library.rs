//! Integration tests: the library use cases with the real fs/JSON/ZIP/SHA-256
//! adapters, always in temporary directories.
#[path = "support/zips.rs"]
mod zips;

use motogp_application::{LibraryError, ModRepository};
use motogp_domain::ModEntry;
use motogp_infrastructure::{mod_library, DataDir, JsonModRepository};
use std::fs;
use std::path::{Path, PathBuf};
use zips::build_zip;

/// SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
const ABC: &[u8] = b"abc";
const ABC_ID: &str = "ba7816bf8f01";

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    src: PathBuf,
    data: DataDir,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Usuário Padrão");
    let src = root.join("downloads");
    fs::create_dir_all(&src).unwrap();
    let data = DataDir::new(root.join("data"));
    Env {
        _tmp: tmp,
        root,
        src,
        data,
    }
}

fn index(env: &Env) -> Vec<ModEntry> {
    JsonModRepository::new(env.data.library_file())
        .list()
        .unwrap()
}

/// Entries of `library/` (mod folders and leftovers), sorted.
fn library_entries(env: &Env) -> Vec<String> {
    let mut names: Vec<String> = match fs::read_dir(env.data.library_dir()) {
        Ok(rd) => rd
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => vec![],
    };
    names.sort();
    names
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn import_single_pak_copies_into_library() {
    let env = env();
    fs::write(env.src.join("A.pak"), ABC).unwrap();

    let entries = mod_library(&env.data)
        .import(&env.src.join("A.pak"))
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "A");
    assert_eq!(entries[0].files, vec!["A.pak"]);
    assert_eq!(entries[0].size_bytes, 3);
    let id = &entries[0].id;
    assert_eq!(
        fs::read(env.data.library_dir().join(id).join("A.pak")).unwrap(),
        ABC
    );
    assert_eq!(index(&env), entries);
    assert!(env.src.join("A.pak").exists(), "the source is left alone");
    assert_eq!(library_entries(&env), vec![id.clone()], "no temp leftovers");
}

#[test]
fn import_pak_picks_up_utoc_ucas_siblings() {
    let env = env();
    for (name, content) in [
        ("A_P.pak", ABC),
        ("A_P.utoc", b"utoc".as_slice()),
        ("A_P.ucas", b"ucas".as_slice()),
        ("B.pak", b"bbb".as_slice()),
        ("A_P.txt", b"notes".as_slice()),
    ] {
        fs::write(env.src.join(name), content).unwrap();
    }

    let entries = mod_library(&env.data)
        .import(&env.src.join("A_P.pak"))
        .unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "A");
    assert_eq!(entries[0].files, vec!["A_P.pak", "A_P.ucas", "A_P.utoc"]);
    assert_eq!(entries[0].size_bytes, 3 + 4 + 4);
    assert_eq!(
        files_in(&env.data.library_dir().join(&entries[0].id)),
        vec!["A_P.pak", "A_P.ucas", "A_P.utoc"]
    );
    assert_eq!(index(&env).len(), 1);
}

#[test]
fn import_zip_with_nested_folders_finds_all_paks() {
    let env = env();
    let zip = env.src.join("mods.zip");
    build_zip(
        &zip,
        &[
            ("x/y/A.pak", ABC),
            ("B.pak", b"bbb"),
            ("x/y/readme.txt", b"hi"),
        ],
    );

    let entries = mod_library(&env.data).import(&zip).unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].files, vec!["A.pak"]);
    assert_eq!(entries[1].files, vec!["B.pak"]);
    assert_eq!(index(&env), entries);
    assert_eq!(library_entries(&env).len(), 2, "no temp leftovers");
}

#[test]
fn import_zip_without_pak_errors() {
    let env = env();
    let zip = env.src.join("docs.zip");
    build_zip(&zip, &[("readme.txt", b"hi")]);

    let error = mod_library(&env.data).import(&zip).unwrap_err();

    assert_eq!(error, LibraryError::NoPakFound);
    assert!(error.to_string().contains("no .pak"));
    assert!(index(&env).is_empty());
    assert!(!env.data.library_file().exists());
    assert!(library_entries(&env).is_empty(), "staging was removed");
}

#[test]
fn remove_deletes_dir_and_index_entry() {
    let env = env();
    fs::write(env.src.join("A.pak"), ABC).unwrap();
    fs::write(env.src.join("B.pak"), b"bbb").unwrap();
    let library = mod_library(&env.data);
    let a = library.import(&env.src.join("A.pak")).unwrap().remove(0);
    let b = library.import(&env.src.join("B.pak")).unwrap().remove(0);

    library.remove(&a.id).unwrap();

    assert!(!env.data.library_dir().join(&a.id).exists());
    assert_eq!(index(&env), vec![b.clone()]);
    assert!(env.data.library_dir().join(&b.id).join("B.pak").exists());
    assert_eq!(
        library.remove(&a.id),
        Err(LibraryError::NotFound { id: a.id.clone() })
    );
}

#[test]
fn remove_rejects_ids_that_could_escape_the_library() {
    let env = env();
    fs::create_dir_all(&env.root.join("data").join("keep")).unwrap();
    for id in ["..", "../keep", "..\\keep", ""] {
        assert!(matches!(
            mod_library(&env.data).remove(id),
            Err(LibraryError::InvalidId { .. })
        ));
    }
    assert!(env.root.join("data").join("keep").exists());
}

#[test]
fn zip_entry_with_parent_traversal_is_ignored() {
    let env = env();
    let zip = env.src.join("evil.zip");
    build_zip(&zip, &[("../evil.pak", b"evil"), ("ok.pak", ABC)]);

    let entries = mod_library(&env.data).import(&zip).unwrap();

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].files, vec!["ok.pak"]);
    assert!(!env.src.join("evil.pak").exists());
    assert!(!env.data.root().join("evil.pak").exists());
    assert!(!env.data.library_dir().join("evil.pak").exists());
    assert!(!env.root.join("evil.pak").exists());
}

#[test]
fn flattened_zip_name_collision_is_rejected() {
    let env = env();
    fs::write(env.src.join("Keep.pak"), b"keep").unwrap();
    let library = mod_library(&env.data);
    let existing = library.import(&env.src.join("Keep.pak")).unwrap();
    let before = fs::read(env.data.library_file()).unwrap();
    let zip = env.src.join("collide.zip");
    build_zip(&zip, &[("x/A.pak", b"one"), ("y/A.pak", b"two")]);

    let error = library.import(&zip).unwrap_err();

    assert!(
        matches!(error, LibraryError::NameCollision { .. }),
        "{error:?}"
    );
    assert_eq!(index(&env), existing, "index untouched");
    assert_eq!(fs::read(env.data.library_file()).unwrap(), before);
    assert_eq!(
        library_entries(&env),
        vec![existing[0].id.clone()],
        "no new folder and no staging leftovers"
    );
    assert_eq!(
        fs::read(
            env.data
                .library_dir()
                .join(&existing[0].id)
                .join("Keep.pak")
        )
        .unwrap(),
        b"keep"
    );
}

#[test]
fn mod_id_matches_first_twelve_sha256_hex() {
    let env = env();
    fs::write(env.src.join("A.pak"), ABC).unwrap();

    let entries = mod_library(&env.data)
        .import(&env.src.join("A.pak"))
        .unwrap();

    assert_eq!(entries[0].id, ABC_ID);
}

#[test]
fn reimport_same_content_returns_the_existing_entry() {
    let env = env();
    fs::write(env.src.join("A.pak"), ABC).unwrap();
    fs::write(env.src.join("Copy.pak"), ABC).unwrap();
    let library = mod_library(&env.data);

    let first = library.import(&env.src.join("A.pak")).unwrap();
    let second = library.import(&env.src.join("Copy.pak")).unwrap();

    assert_eq!(first, second);
    assert_eq!(index(&env).len(), 1);
    assert_eq!(library_entries(&env), vec![ABC_ID.to_string()]);
}

#[test]
fn unsupported_format_is_rejected_before_touching_the_library() {
    let env = env();
    fs::write(env.src.join("mod.rar"), b"rar").unwrap();

    let error = mod_library(&env.data)
        .import(&env.src.join("mod.rar"))
        .unwrap_err();

    assert!(error.to_string().contains("unsupported format"));
    assert!(!env.data.library_dir().exists());
    assert!(!env.data.library_file().exists());
}

#[test]
fn corrupted_index_blocks_the_import_without_losing_the_files() {
    let env = env();
    fs::write(env.src.join("A.pak"), ABC).unwrap();
    fs::create_dir_all(env.data.root()).unwrap();
    fs::write(env.data.library_file(), "{ not json").unwrap();

    let error = mod_library(&env.data)
        .import(&env.src.join("A.pak"))
        .unwrap_err();

    assert!(matches!(error, LibraryError::Storage(_)), "{error:?}");
    assert_eq!(
        fs::read_to_string(env.data.library_file()).unwrap(),
        "{ not json"
    );
    assert!(
        !env.data.library_dir().join(ABC_ID).exists(),
        "the published folder was rolled back"
    );
}
