use crate::{GameError, GameFiles, SteamLocator};
use motogp_domain::GameInfo;
use std::path::Path;

/// Use cases: detect Steam installations and inspect a manually chosen root.
pub struct GameService<L: SteamLocator, F: GameFiles> {
    locator: L,
    files: F,
}

impl<L: SteamLocator, F: GameFiles> GameService<L, F> {
    pub fn new(locator: L, files: F) -> Self {
        Self { locator, files }
    }

    /// MotoGP installations across all Steam libraries. No Steam: empty list.
    pub fn detect_games(&self) -> Result<Vec<GameInfo>, GameError> {
        let _ = (&self.locator, &self.files);
        todo!()
    }

    /// Validates `root` as an Unreal game; creates nothing on disk.
    pub fn inspect_root(&self, root: &Path) -> Result<GameInfo, GameError> {
        let _ = (&self.locator, &self.files);
        todo!("{root:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppError;
    use motogp_domain::SteamManifest;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[derive(Default)]
    struct FakeFiles {
        dirs: BTreeSet<PathBuf>,
        manifests: Vec<SteamManifest>,
    }

    impl FakeFiles {
        fn with_dir(mut self, path: &str) -> Self {
            let mut p = PathBuf::from(path);
            loop {
                self.dirs.insert(p.clone());
                if !p.pop() || p.as_os_str().is_empty() {
                    break;
                }
            }
            self
        }
        fn with_manifest(mut self, app_id: u32, install_dir: &str) -> Self {
            self.manifests.push(SteamManifest {
                app_id,
                install_dir: install_dir.into(),
            });
            self
        }
        fn children(&self, dir: &Path) -> Vec<PathBuf> {
            self.dirs
                .iter()
                .filter(|d| d.parent() == Some(dir))
                .cloned()
                .collect()
        }
    }

    impl GameFiles for FakeFiles {
        fn common_dirs(&self, library: &Path) -> Result<Vec<PathBuf>, AppError> {
            Ok(self.children(&library.join("steamapps").join("common")))
        }
        fn subdirs(&self, dir: &Path) -> Result<Vec<String>, AppError> {
            Ok(self
                .children(dir)
                .iter()
                .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
                .collect())
        }
        fn has_paks(&self, project_dir: &Path) -> bool {
            self.dirs
                .contains(&project_dir.join("Content").join("Paks"))
        }
        fn manifests(&self, _steamapps: &Path) -> Result<Vec<SteamManifest>, AppError> {
            Ok(self.manifests.clone())
        }
    }

    struct FakeLocator(Vec<PathBuf>);
    impl SteamLocator for FakeLocator {
        fn library_folders(&self) -> Result<Vec<PathBuf>, AppError> {
            Ok(self.0.clone())
        }
    }

    fn service(libs: &[&str], files: FakeFiles) -> GameService<FakeLocator, FakeFiles> {
        GameService::new(FakeLocator(libs.iter().map(PathBuf::from).collect()), files)
    }

    #[test]
    fn missing_steam_returns_empty_list() {
        let svc = service(&[], FakeFiles::default());
        assert_eq!(svc.detect_games(), Ok(vec![]));
    }

    #[test]
    fn manual_game_without_manifest_has_no_app_id() {
        let files = FakeFiles::default().with_dir("games/MotoGP25/MotoGP25/Content/Paks");
        let info = service(&[], files)
            .inspect_root(Path::new("games/MotoGP25"))
            .unwrap();
        assert_eq!(info.app_id, None);
        assert!(info.mods_dir.ends_with("MotoGP25/Content/Paks/~mods"));
    }

    #[test]
    fn engine_folder_is_not_selected() {
        let files = FakeFiles::default()
            .with_dir("g/MotoGP25/Engine/Content/Paks")
            .with_dir("g/MotoGP25/Bike/Content/Paks");
        let info = service(&[], files)
            .inspect_root(Path::new("g/MotoGP25"))
            .unwrap();
        assert!(info.mods_dir.starts_with(Path::new("g/MotoGP25/Bike")));
    }

    #[test]
    fn manifest_match_is_case_insensitive() {
        let files = FakeFiles::default()
            .with_dir("lib/steamapps/common/MotoGP25/MotoGP25/Content/Paks")
            .with_manifest(123, "motogp25");
        let info = service(&["lib"], files)
            .inspect_root(Path::new("lib/steamapps/common/MotoGP25"))
            .unwrap();
        assert_eq!(info.app_id, Some(123));
    }

    #[test]
    fn root_without_paks_is_not_an_unreal_game() {
        let files = FakeFiles::default().with_dir("g/Empty/Stuff");
        let err = service(&[], files)
            .inspect_root(Path::new("g/Empty"))
            .unwrap_err();
        assert!(matches!(err, GameError::NotUnrealGame { .. }));
    }

    #[test]
    fn detect_games_returns_only_valid_motogp_folders() {
        let files = FakeFiles::default()
            .with_dir("lib/steamapps/common/MotoGP25/MotoGP25/Content/Paks")
            .with_dir("lib/steamapps/common/OtherGame/Other/Content/Paks")
            .with_dir("lib/steamapps/common/MotoGPBroken/Nothing");
        let games = service(&["lib"], files).detect_games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "MotoGP25");
    }

    #[test]
    fn storage_errors_are_wrapped() {
        struct Failing;
        impl SteamLocator for Failing {
            fn library_folders(&self) -> Result<Vec<PathBuf>, AppError> {
                Err(AppError::Read {
                    location: "x".into(),
                    detail: "d".into(),
                })
            }
        }
        let svc = GameService::new(Failing, FakeFiles::default());
        assert!(matches!(svc.detect_games(), Err(GameError::Storage(_))));
    }

    #[test]
    fn error_display_says_content_paks_not_found() {
        let err = GameError::NotUnrealGame {
            root: PathBuf::from("g/Empty"),
        };
        let text = err.to_string();
        assert!(text.contains("not an Unreal game: Content/Paks not found"));
        assert!(text.contains("Empty"));
    }
}
