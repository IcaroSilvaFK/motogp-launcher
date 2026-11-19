//! In-memory fakes for config, profiles, Steam and game files (issue #8).
#![allow(dead_code)]
use motogp_application::{
    AppError, Clock, ConfigRepository, GameFiles, GameService, GameSettings, ProfileRepository,
    ProfileService, SteamLocator,
};
use motogp_domain::{Config, Profile, SteamManifest};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
pub struct ConfigState {
    pub config: Config,
    pub saves: usize,
    pub fail_save: bool,
}

#[derive(Clone, Default)]
pub struct FakeConfig(pub Rc<RefCell<ConfigState>>);

impl ConfigRepository for FakeConfig {
    fn load(&self) -> Result<Config, AppError> {
        Ok(self.0.borrow().config.clone())
    }

    fn save(&self, config: &Config) -> Result<(), AppError> {
        let mut state = self.0.borrow_mut();
        if state.fail_save {
            return Err(AppError::Write {
                location: "config.json".into(),
                detail: "disk full".into(),
            });
        }
        state.saves += 1;
        state.config = config.clone();
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct FakeProfiles(pub Rc<RefCell<Vec<Profile>>>);

impl ProfileRepository for FakeProfiles {
    fn load(&self) -> Result<Vec<Profile>, AppError> {
        Ok(self.0.borrow().clone())
    }

    fn save(&self, profiles: &[Profile]) -> Result<(), AppError> {
        *self.0.borrow_mut() = profiles.to_vec();
        Ok(())
    }
}

pub struct FixedClock(pub u64);

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        self.0
    }
}

pub fn profile_service(profiles: &FakeProfiles) -> ProfileService<FakeProfiles, FixedClock> {
    ProfileService::new(profiles.clone(), FixedClock(1_000))
}

pub struct NoSteam;

impl SteamLocator for NoSteam {
    fn library_folders(&self) -> Result<Vec<PathBuf>, AppError> {
        Ok(vec![])
    }
}

/// Only the roots listed here are valid Unreal installations.
#[derive(Clone, Default)]
pub struct FakeGameFiles {
    pub valid_roots: Vec<PathBuf>,
    pub manifests_fail: bool,
}

impl GameFiles for FakeGameFiles {
    fn common_dirs(&self, _library: &Path) -> Result<Vec<PathBuf>, AppError> {
        Ok(vec![])
    }

    fn subdirs(&self, dir: &Path) -> Result<Vec<String>, AppError> {
        Ok(if self.valid_roots.iter().any(|r| r == dir) {
            vec!["MotoGP25".to_string()]
        } else {
            vec![]
        })
    }

    fn has_paks(&self, project_dir: &Path) -> bool {
        project_dir
            .parent()
            .is_some_and(|root| self.valid_roots.iter().any(|r| r == root))
    }

    fn manifests(&self, _steamapps: &Path) -> Result<Vec<SteamManifest>, AppError> {
        if self.manifests_fail {
            return Err(AppError::Read {
                location: "appmanifest_1.acf".into(),
                detail: "denied".into(),
            });
        }
        Ok(vec![SteamManifest {
            app_id: 123,
            install_dir: "MotoGP25".into(),
        }])
    }
}

pub fn valid_root() -> PathBuf {
    PathBuf::from("lib")
        .join("steamapps")
        .join("common")
        .join("MotoGP25")
}

pub type Settings = GameSettings<FakeConfig, NoSteam, FakeGameFiles>;

pub fn settings(config: &FakeConfig, files: FakeGameFiles) -> Settings {
    GameSettings::new(config.clone(), GameService::new(NoSteam, files))
}
