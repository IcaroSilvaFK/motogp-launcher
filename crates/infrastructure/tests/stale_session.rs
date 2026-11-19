//! Stale-manifest recovery (issue #23) against a temporary file system with two
//! "game installs", each with its own `~mods` folder.
use motogp_application::{
    DeployError, DeployMod, DeployService, DeploySession, DeployedRepository,
};
use motogp_domain::Deployed;
use motogp_infrastructure::{FsDeployFiles, JsonDeployedRepository};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const ID_A: &str = "aaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbb";

struct Env {
    _tmp: TempDir,
    lib: PathBuf,
    /// `~mods` of the previous game install (where the manifest points).
    old: PathBuf,
    /// `~mods` of the current game install (the authorized one).
    current: PathBuf,
    manifest: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    // Accented folder on purpose: real Windows users have them.
    let root = tmp.path().join("Usuário Padrão");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).unwrap();
    Env {
        old: root.join("install-old").join("~mods"),
        current: root.join("install-new").join("~mods"),
        manifest: root.join("data").join("deployed.json"),
        lib,
        _tmp: tmp,
    }
}

impl Env {
    fn service(&self) -> DeployService<JsonDeployedRepository, FsDeployFiles> {
        DeployService::new(
            JsonDeployedRepository::new(&self.manifest),
            FsDeployFiles::new(),
        )
    }

    fn repo(&self) -> JsonDeployedRepository {
        JsonDeployedRepository::new(&self.manifest)
    }

    fn deploy_mod(&self, id: &str, name: &str) -> DeployMod {
        let dir = self.lib.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), name.as_bytes()).unwrap();
        DeployMod {
            id: id.into(),
            source_dir: dir,
            files: vec![name.into()],
        }
    }

    /// Deploys two mods into the OLD install, as if it was the configured one.
    fn deploy_into_old(&self) -> Vec<String> {
        let mods = [
            self.deploy_mod(ID_B, "B.pak"),
            self.deploy_mod(ID_A, "A.pak"),
        ];
        let names = self.service().deploy(&self.old, &mods).unwrap();
        // The user then switched the game install: the new `~mods` exists too.
        fs::create_dir_all(&self.current).unwrap();
        names
    }

    fn names(dir: &PathBuf) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|r| {
                r.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

#[test]
fn a_manifest_of_another_install_is_reported_stale_without_changes() {
    let env = env();
    let names = env.deploy_into_old();
    let manifest = env.repo().load().unwrap();

    let session = env.service().session(&env.current).unwrap();

    assert_eq!(
        session,
        DeploySession::Stale {
            mods_dir: env.old.clone(),
            tracked: 2
        }
    );
    assert_eq!(env.repo().load().unwrap(), manifest);
    assert_eq!(Env::names(&env.old).len(), names.len());
    assert!(Env::names(&env.current).is_empty());
}

#[test]
fn session_is_none_current_or_vanished_as_the_manifest_dictates() {
    let env = env();
    assert_eq!(
        env.service().session(&env.current).unwrap(),
        DeploySession::None
    );

    env.deploy_into_old();
    assert_eq!(
        env.service().session(&env.old).unwrap(),
        DeploySession::Current { tracked: 2 }
    );

    fs::remove_dir_all(&env.old).unwrap();
    assert_eq!(
        env.service().session(&env.current).unwrap(),
        DeploySession::Vanished {
            mods_dir: env.old.clone()
        }
    );
}

#[test]
fn forget_clears_the_manifest_and_deletes_nothing() {
    let env = env();
    let names = env.deploy_into_old();

    env.service().forget_stale_session(&env.current).unwrap();

    assert_eq!(env.repo().load().unwrap(), Deployed::default());
    assert_eq!(Env::names(&env.old), {
        let mut n = names.clone();
        n.sort();
        n
    });
}

#[test]
fn forget_refuses_the_current_install_and_keeps_the_manifest() {
    let env = env();
    env.deploy_into_old();
    let manifest = env.repo().load().unwrap();

    let error = env.service().forget_stale_session(&env.old).unwrap_err();

    assert!(matches!(error, DeployError::SessionNotStale { .. }));
    assert_eq!(env.repo().load().unwrap(), manifest);
}

#[test]
fn confirmed_clean_removes_only_our_files_of_the_old_folder() {
    let env = env();
    env.deploy_into_old();
    fs::write(env.old.join("meu_mod.pak"), b"manual").unwrap();

    let removed = env
        .service()
        .clean_stale_session(&env.current, &env.old)
        .unwrap();

    assert_eq!(removed, 2);
    assert_eq!(Env::names(&env.old), vec!["meu_mod.pak"]);
    assert_eq!(fs::read(env.old.join("meu_mod.pak")).unwrap(), b"manual");
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn a_tracked_file_replaced_by_the_user_is_kept_and_untracked() {
    let env = env();
    let names = env.deploy_into_old();
    fs::remove_file(env.old.join(&names[0])).unwrap();
    fs::write(env.old.join(&names[0]), b"a user file").unwrap();

    let removed = env
        .service()
        .clean_stale_session(&env.current, &env.old)
        .unwrap();

    assert_eq!(removed, 1);
    assert_eq!(fs::read(env.old.join(&names[0])).unwrap(), b"a user file");
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn a_confirmation_naming_another_folder_is_refused() {
    let env = env();
    env.deploy_into_old();
    let manifest = env.repo().load().unwrap();

    let error = env
        .service()
        .clean_stale_session(&env.current, &env.current)
        .unwrap_err();

    assert!(matches!(error, DeployError::UnauthorizedDirectory { .. }));
    assert_eq!(Env::names(&env.old).len(), 2);
    assert_eq!(env.repo().load().unwrap(), manifest);
}

#[test]
fn a_confirmation_with_dot_dot_that_resolves_to_the_recorded_folder_is_accepted() {
    let env = env();
    env.deploy_into_old();
    // `.../install-old/~mods/../~mods` canonicalizes to the recorded folder.
    let confirmed = env.old.join("..").join("~mods");

    let removed = env
        .service()
        .clean_stale_session(&env.current, &confirmed)
        .unwrap();

    assert_eq!(removed, 2);
}

#[test]
fn a_recorded_folder_not_named_mods_is_refused() {
    let env = env();
    let renamed = env.old.parent().unwrap().join("Documents");
    let mods = [env.deploy_mod(ID_A, "A.pak")];
    let names = env.service().deploy(&renamed, &mods).unwrap();
    fs::create_dir_all(&env.current).unwrap();

    let error = env
        .service()
        .clean_stale_session(&env.current, &renamed)
        .unwrap_err();

    assert!(matches!(error, DeployError::UnauthorizedDirectory { .. }));
    assert!(renamed.join(&names[0]).exists());
    assert!(!env.repo().load().unwrap().is_empty());
}

#[test]
fn a_vanished_old_folder_is_cleared_without_an_error() {
    let env = env();
    env.deploy_into_old();
    fs::remove_dir_all(&env.old).unwrap();

    let removed = env
        .service()
        .clean_stale_session(&env.current, &env.old)
        .unwrap();

    assert_eq!(removed, 0);
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn clean_stale_session_on_the_current_install_behaves_like_clean() {
    let env = env();
    env.deploy_into_old();

    let removed = env
        .service()
        .clean_stale_session(&env.old, &env.current)
        .unwrap();

    assert_eq!(removed, 2);
    assert!(Env::names(&env.old).is_empty());
    assert_eq!(env.repo().load().unwrap(), Deployed::default());
}

#[test]
fn deploy_works_again_after_forgetting_the_old_session() {
    let env = env();
    env.deploy_into_old();
    assert!(env
        .service()
        .deploy(&env.current, &[env.deploy_mod(ID_A, "A.pak")])
        .is_err());

    env.service().forget_stale_session(&env.current).unwrap();
    let names = env
        .service()
        .deploy(&env.current, &[env.deploy_mod(ID_A, "A.pak")])
        .unwrap();

    assert_eq!(names, vec!["000_aaaaaaaaaaaa_A.pak"]);
    assert_eq!(Env::names(&env.old).len(), 2);
}
