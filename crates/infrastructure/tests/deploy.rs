//! Adapter contracts of deploy/clean against a temporary file system.
use motogp_application::{
    AppError, DeployError, DeployFiles, DeployMod, DeployService, DeployedRepository,
};
use motogp_domain::Deployed;
use motogp_infrastructure::{FsDeployFiles, JsonDeployedRepository};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const ID_A: &str = "aaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbb";

struct Env {
    _tmp: TempDir,
    lib: PathBuf,
    mods: PathBuf,
    manifest: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("Usuário Padrão");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).unwrap();
    Env {
        mods: root.join("game").join("~mods"),
        manifest: root.join("data").join("deployed.json"),
        lib,
        _tmp: tmp,
    }
}

impl Env {
    fn source(&self, id: &str, name: &str, content: &[u8]) -> PathBuf {
        let dir = self.lib.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
        dir.join(name)
    }

    fn deploy_mod(&self, id: &str, name: &str) -> DeployMod {
        self.source(id, name, name.as_bytes());
        DeployMod {
            id: id.into(),
            source_dir: self.lib.join(id),
            files: vec![name.into()],
        }
    }

    fn service(&self) -> DeployService<JsonDeployedRepository, FsDeployFiles> {
        DeployService::new(
            JsonDeployedRepository::new(&self.manifest),
            FsDeployFiles::new(),
        )
    }

    fn repo(&self) -> JsonDeployedRepository {
        JsonDeployedRepository::new(&self.manifest)
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.mods)
            .map(|r| {
                r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

fn failing_link(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "cross-volume"))
}

fn deploy_two(env: &Env) -> Vec<String> {
    let mods = [env.deploy_mod(ID_B, "B.pak"), env.deploy_mod(ID_A, "A.pak")];
    env.service().deploy(&env.mods, &mods).unwrap()
}

// ---------- the seven original names + additions ----------

#[test]
fn clean_removes_only_deployed_files() {
    let env = env();
    deploy_two(&env);
    fs::write(env.mods.join("meu_mod.pak"), b"manual").unwrap();

    let removed = env.service().clean().unwrap();

    assert_eq!(removed, 2);
    assert_eq!(env.names(), vec!["meu_mod.pak"]);
    assert_eq!(fs::read(env.mods.join("meu_mod.pak")).unwrap(), b"manual");
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn clean_tolerates_already_missing_file() {
    let env = env();
    let names = deploy_two(&env);
    fs::remove_file(env.mods.join(&names[0])).unwrap();

    assert_eq!(env.service().clean().unwrap(), 1);
    assert!(env.repo().load().unwrap().is_empty());
}

#[test]
fn clean_tolerates_a_missing_mods_directory() {
    let env = env();
    deploy_two(&env);
    fs::remove_dir_all(&env.mods).unwrap();

    assert_eq!(env.service().clean().unwrap(), 0);
    assert!(env.repo().load().unwrap().is_empty());
}

#[test]
fn deploy_refuses_to_overwrite_foreign_file() {
    let env = env();
    fs::create_dir_all(&env.mods).unwrap();
    fs::write(env.mods.join("000_aaaaaaaaaaaa_A.pak"), b"manual").unwrap();
    let mods = [env.deploy_mod(ID_A, "A.pak")];

    let error = env.service().deploy(&env.mods, &mods).unwrap_err();

    assert!(error.to_string().contains("already exists"), "{error}");
    assert_eq!(
        fs::read(env.mods.join("000_aaaaaaaaaaaa_A.pak")).unwrap(),
        b"manual"
    );
    assert!(env.repo().load().unwrap().is_empty());
}

#[test]
fn adapter_refuses_to_overwrite_an_existing_destination_by_hard_link() {
    let env = env();
    let source = env.source(ID_A, "A.pak", b"new");
    fs::create_dir_all(&env.mods).unwrap();
    fs::write(env.mods.join("x.pak"), b"manual").unwrap();

    let error = FsDeployFiles::new()
        .materialize(&source, &env.mods, "x.pak")
        .unwrap_err();

    assert!(matches!(error, DeployError::AlreadyExists { .. }));
    assert_eq!(fs::read(env.mods.join("x.pak")).unwrap(), b"manual");
}

#[test]
fn copy_fallback_never_overwrites_existing_file() {
    let env = env();
    let source = env.source(ID_A, "A.pak", b"new");
    fs::create_dir_all(&env.mods).unwrap();
    fs::write(env.mods.join("x.pak"), b"manual").unwrap();

    let error = FsDeployFiles::with_link(failing_link)
        .materialize(&source, &env.mods, "x.pak")
        .unwrap_err();

    assert!(matches!(error, DeployError::AlreadyExists { .. }), "{error}");
    assert_eq!(fs::read(env.mods.join("x.pak")).unwrap(), b"manual");
}

#[test]
fn hard_link_failure_falls_back_to_copy() {
    let env = env();
    let source = env.source(ID_A, "A.pak", b"content");
    fs::create_dir_all(&env.mods).unwrap();

    FsDeployFiles::with_link(failing_link)
        .materialize(&source, &env.mods, "x.pak")
        .unwrap();

    assert_eq!(fs::read(env.mods.join("x.pak")).unwrap(), b"content");
    // A real copy: changing the source afterwards does not change the copy.
    fs::OpenOptions::new()
        .append(true)
        .open(&source)
        .unwrap()
        .write_all(b"!")
        .unwrap();
    assert_eq!(fs::read(env.mods.join("x.pak")).unwrap(), b"content");
}

#[test]
fn hard_link_is_used_when_it_succeeds() {
    let env = env();
    let source = env.source(ID_A, "A.pak", b"content");
    fs::create_dir_all(&env.mods).unwrap();

    FsDeployFiles::new()
        .materialize(&source, &env.mods, "x.pak")
        .unwrap();

    // Same file on disk: an append through the source is visible in the link.
    fs::OpenOptions::new()
        .append(true)
        .open(&source)
        .unwrap()
        .write_all(b"!")
        .unwrap();
    assert_eq!(fs::read(env.mods.join("x.pak")).unwrap(), b"content!");
    // Removing the deployed link leaves the library file untouched.
    assert!(FsDeployFiles::new().remove(&env.mods, "x.pak").unwrap());
    assert_eq!(fs::read(&source).unwrap(), b"content!");
}

#[test]
fn a_failed_copy_leaves_no_partial_destination() {
    let env = env();
    fs::create_dir_all(&env.mods).unwrap();

    let error = FsDeployFiles::with_link(failing_link)
        .materialize(&env.lib.join("missing.pak"), &env.mods, "x.pak")
        .unwrap_err();

    assert!(matches!(error, DeployError::Io { .. }), "{error}");
    assert!(!env.mods.join("x.pak").exists());
}

#[test]
fn adapter_rejects_names_that_could_escape_the_directory() {
    let env = env();
    fs::create_dir_all(&env.mods).unwrap();
    let outside = env.mods.parent().unwrap().join("evil.pak");
    fs::write(&outside, b"external").unwrap();
    let files = FsDeployFiles::new();
    let source = env.source(ID_A, "A.pak", b"x");

    for bad in ["../evil.pak", "..\\evil.pak", "sub/x.pak", "", ".."] {
        assert!(matches!(
            files.remove(&env.mods, bad),
            Err(DeployError::InvalidPath { .. })
        ));
        assert!(matches!(
            files.exists(&env.mods, bad),
            Err(DeployError::InvalidPath { .. })
        ));
        assert!(matches!(
            files.materialize(&source, &env.mods, bad),
            Err(DeployError::InvalidPath { .. })
        ));
    }
    assert_eq!(fs::read(outside).unwrap(), b"external");
}

#[test]
fn exists_and_create_dir_all_behave_like_the_filesystem() {
    let env = env();
    let files = FsDeployFiles::new();
    assert!(!files.exists(&env.mods, "x.pak").unwrap());
    files.create_dir_all(&env.mods).unwrap();
    fs::write(env.mods.join("x.pak"), b"1").unwrap();
    assert!(files.exists(&env.mods, "x.pak").unwrap());
}

#[test]
fn remove_of_a_missing_file_reports_false() {
    let env = env();
    fs::create_dir_all(&env.mods).unwrap();
    assert!(!FsDeployFiles::new().remove(&env.mods, "nope.pak").unwrap());
}

// ---------- end to end on the real fs ----------

#[test]
fn deploy_writes_files_in_profile_order_with_prefix_on_disk() {
    let env = env();

    let names = deploy_two(&env);

    assert_eq!(
        names,
        vec!["000_bbbbbbbbbbbb_B.pak", "001_aaaaaaaaaaaa_A.pak"]
    );
    assert_eq!(env.names(), names);
    assert_eq!(env.repo().load().unwrap().files, names);
}

#[test]
fn deploy_empty_profile_does_not_create_the_directory() {
    let env = env();

    env.service().deploy(&env.mods, &[]).unwrap();

    assert!(!env.mods.exists());
}

#[test]
fn failed_clean_on_disk_keeps_pending_entries_for_a_retry() {
    let env = env();
    let names = deploy_two(&env);
    let second = env.mods.join(&names[1]);
    fs::remove_file(&second).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(second.join("inner"), b"x").unwrap();

    let error = env.service().clean().unwrap_err();

    assert!(matches!(error, DeployError::CleanIncomplete { .. }));
    assert_eq!(env.repo().load().unwrap().files, vec![names[1].clone()]);
    fs::remove_dir_all(&second).unwrap();
    assert_eq!(env.service().clean().unwrap(), 0);
    assert!(env.repo().load().unwrap().is_empty());
}

#[test]
fn invalid_manifest_path_on_disk_is_rejected() {
    let env = env();
    fs::create_dir_all(&env.mods).unwrap();
    let outside = env.mods.parent().unwrap().join("evil.pak");
    fs::write(&outside, b"external").unwrap();
    env.repo()
        .save(&Deployed {
            mods_dir: env.mods.clone(),
            files: vec!["../evil.pak".into()],
            pending: vec![],
        })
        .unwrap();

    let error = env.service().clean().unwrap_err();

    assert!(matches!(error, DeployError::InvalidPath { .. }));
    assert_eq!(fs::read(outside).unwrap(), b"external");
}

// ---------- manifest repository ----------

#[test]
fn missing_manifest_loads_as_default() {
    let env = env();
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn manifest_roundtrips_files_and_pending_entries() {
    let env = env();
    let deployed = Deployed {
        mods_dir: env.mods.clone(),
        files: vec!["a".into()],
        pending: vec!["b".into()],
    };

    env.repo().save(&deployed).unwrap();

    assert_eq!(env.repo().load().unwrap(), deployed);
}

#[test]
fn corrupted_manifest_is_an_error_and_is_not_overwritten() {
    let env = env();
    fs::create_dir_all(env.manifest.parent().unwrap()).unwrap();
    fs::write(&env.manifest, b"{ not json").unwrap();

    let error = env.repo().load().unwrap_err();

    assert!(matches!(
        error,
        DeployError::Storage(AppError::Corrupted { .. })
    ));
    assert_eq!(fs::read(&env.manifest).unwrap(), b"{ not json");
}
