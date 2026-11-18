//! Read-only filesystem adapter of the `GameFiles` port.
use motogp_application::{AppError, GameFiles};
use motogp_domain::SteamManifest;
use std::path::{Path, PathBuf};

/// Reads the real disk; never creates or modifies anything.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsGameFiles;

impl GameFiles for FsGameFiles {
    fn common_dirs(&self, library: &Path) -> Result<Vec<PathBuf>, AppError> {
        todo!("{library:?}")
    }
    fn subdirs(&self, dir: &Path) -> Result<Vec<String>, AppError> {
        todo!("{dir:?}")
    }
    fn has_paks(&self, project_dir: &Path) -> bool {
        todo!("{project_dir:?}")
    }
    fn manifests(&self, steamapps: &Path) -> Result<Vec<SteamManifest>, AppError> {
        todo!("{steamapps:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SteamLibraries;
    use motogp_application::{GameError, GameService};
    use std::fs;
    use tempfile::TempDir;

    fn service(steam: &Path) -> GameService<SteamLibraries, FsGameFiles> {
        GameService::new(SteamLibraries::new(Some(steam.to_path_buf())), FsGameFiles)
    }

    fn mkdirs(base: &Path, rel: &str) -> PathBuf {
        let p = base.join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn manifest(base: &Path, id: u32, install_dir: &str) {
        let acf = format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"installdir\"\t\t\"{install_dir}\"\n}}"
        );
        let dir = mkdirs(base, "steamapps");
        fs::write(dir.join(format!("appmanifest_{id}.acf")), acf).unwrap();
    }

    fn steam() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn inspect_game_finds_mods_dir_and_app_id() {
        let tmp = steam();
        let root = mkdirs(tmp.path(), "steamapps/common/MotoGP25");
        mkdirs(&root, "MotoGP25/Content/Paks");
        manifest(tmp.path(), 123, "motogp25");
        let info = service(tmp.path()).inspect_root(&root).unwrap();
        assert_eq!(info.app_id, Some(123));
        assert_eq!(info.name, "MotoGP25");
        assert!(info.mods_dir.ends_with("MotoGP25/Content/Paks/~mods"));
    }

    #[test]
    fn inspect_game_without_paks_errors() {
        let tmp = steam();
        let root = mkdirs(tmp.path(), "steamapps/common/MotoGP25");
        mkdirs(&root, "MotoGP25/Content");
        let err = service(tmp.path()).inspect_root(&root).unwrap_err();
        assert!(matches!(err, GameError::NotUnrealGame { .. }));
        assert!(err.to_string().contains("Content/Paks not found"));
    }

    #[test]
    fn detect_games_only_returns_motogp_folders() {
        let tmp = steam();
        let common = mkdirs(tmp.path(), "steamapps/common");
        mkdirs(&common, "MotoGP25/MotoGP25/Content/Paks");
        mkdirs(&common, "OtherGame/Other/Content/Paks");
        let games = service(tmp.path()).detect_games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "MotoGP25");
    }

    #[test]
    fn inspection_does_not_create_mods_directory() {
        let tmp = steam();
        let root = mkdirs(tmp.path(), "steamapps/common/MotoGP25");
        mkdirs(&root, "MotoGP25/Content/Paks");
        let info = service(tmp.path()).inspect_root(&root).unwrap();
        assert!(!info.mods_dir.exists());
        service(tmp.path()).detect_games().unwrap();
        assert!(!info.mods_dir.exists());
    }

    #[test]
    fn engine_folder_is_skipped_on_disk() {
        let tmp = steam();
        let root = mkdirs(tmp.path(), "MotoGP25");
        mkdirs(&root, "Engine/Content/Paks");
        mkdirs(&root, "Bike/Content/Paks");
        let info = service(tmp.path()).inspect_root(&root).unwrap();
        assert!(info.mods_dir.starts_with(root.join("Bike")));
    }

    #[test]
    fn missing_directories_read_as_empty() {
        let tmp = steam();
        let missing = tmp.path().join("nope");
        assert_eq!(FsGameFiles.common_dirs(&missing), Ok(vec![]));
        assert_eq!(FsGameFiles.subdirs(&missing), Ok(vec![]));
        assert_eq!(FsGameFiles.manifests(&missing), Ok(vec![]));
        assert!(!FsGameFiles.has_paks(&missing));
    }

    #[test]
    fn manifests_ignore_unrelated_and_malformed_files() {
        let tmp = steam();
        let dir = mkdirs(tmp.path(), "steamapps");
        manifest(tmp.path(), 7, "Seven");
        fs::write(dir.join("appmanifest_8.acf"), "garbage").unwrap();
        fs::write(dir.join("libraryfolders.vdf"), "x").unwrap();
        let found = FsGameFiles.manifests(&dir).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].app_id, 7);
    }

    #[test]
    fn unreadable_directory_is_a_read_error() {
        let tmp = steam();
        let file = tmp.path().join("file.txt");
        fs::write(&file, "x").unwrap();
        assert!(matches!(
            FsGameFiles.subdirs(&file),
            Err(AppError::Read { .. })
        ));
    }
}
