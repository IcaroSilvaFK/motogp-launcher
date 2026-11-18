//! Game installation entities and pure selection rules (no fs, registry or serde).
use std::path::{Path, PathBuf};

/// A detected (or manually chosen) game installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInfo {
    /// Name of the installation root folder (e.g. `MotoGP25`).
    pub name: String,
    pub root: PathBuf,
    /// `<root>/<Project>/Content/Paks/~mods`; never created by detection.
    pub mods_dir: PathBuf,
    /// Steam App ID; absent for manual selections without a matching manifest.
    pub app_id: Option<u32>,
}

/// The facts we need from a Steam `appmanifest_*.acf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamManifest {
    pub app_id: u32,
    pub install_dir: String,
}

impl GameInfo {
    /// Builds the info for `root` whose Unreal project folder is `project`.
    pub fn new(root: PathBuf, project: &str, app_id: Option<u32>) -> Self {
        todo!("{root:?} {project} {app_id:?}")
    }
}

/// A folder in `steamapps/common` is a MotoGP candidate when it starts with
/// `MotoGP`, ignoring case.
pub fn is_motogp_folder(name: &str) -> bool {
    todo!("{name}")
}

/// `installdir` from a manifest matches the folder name ignoring case.
pub fn install_dir_matches(install_dir: &str, folder: &str) -> bool {
    todo!("{install_dir} {folder}")
}

/// The Unreal `Engine` folder is never the game project.
pub fn is_engine_folder(name: &str) -> bool {
    todo!("{name}")
}

/// `<root>/<project>/Content/Paks/~mods`.
pub fn mods_dir_for(root: &Path, project: &str) -> PathBuf {
    todo!("{root:?} {project}")
}

/// App ID of the manifest whose `installdir` matches `folder`.
pub fn find_app_id(manifests: &[SteamManifest], folder: &str) -> Option<u32> {
    todo!("{} {folder}", manifests.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motogp_prefix_is_case_insensitive() {
        assert!(is_motogp_folder("MotoGP25"));
        assert!(is_motogp_folder("motogp24"));
        assert!(!is_motogp_folder("OtherGame"));
        assert!(!is_motogp_folder("My MotoGP"));
        assert!(!is_motogp_folder(""));
    }

    #[test]
    fn install_dir_ignores_case() {
        assert!(install_dir_matches("motogp25", "MotoGP25"));
        assert!(!install_dir_matches("MotoGP24", "MotoGP25"));
    }

    #[test]
    fn engine_folder_ignores_case() {
        assert!(is_engine_folder("Engine"));
        assert!(is_engine_folder("engine"));
        assert!(!is_engine_folder("MotoGP25"));
    }

    #[test]
    fn mods_dir_is_under_project_paks() {
        let dir = mods_dir_for(Path::new("root"), "MotoGP25");
        assert_eq!(
            dir,
            Path::new("root")
                .join("MotoGP25")
                .join("Content")
                .join("Paks")
                .join("~mods")
        );
    }

    #[test]
    fn find_app_id_matches_installdir_ignoring_case() {
        let manifests = [
            SteamManifest {
                app_id: 1,
                install_dir: "Other".into(),
            },
            SteamManifest {
                app_id: 123,
                install_dir: "motogp25".into(),
            },
        ];
        assert_eq!(find_app_id(&manifests, "MotoGP25"), Some(123));
        assert_eq!(find_app_id(&manifests, "MotoGP26"), None);
    }

    #[test]
    fn game_info_new_derives_name_and_mods_dir() {
        let info = GameInfo::new(PathBuf::from("lib").join("MotoGP25"), "MotoGP25", Some(5));
        assert_eq!(info.name, "MotoGP25");
        assert_eq!(info.app_id, Some(5));
        assert!(info.mods_dir.ends_with("Paks/~mods"));
    }
}
