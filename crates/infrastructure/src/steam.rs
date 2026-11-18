//! Steam file formats (VDF/ACF) and the Steam library locator adapter.
use motogp_application::{AppError, SteamLocator};
use motogp_domain::SteamManifest;
use std::path::PathBuf;

/// Library roots listed in `libraryfolders.vdf`, with VDF escapes resolved
/// (`\\` becomes `\`). Handles both the modern (`"path"`) and legacy formats.
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    todo!("{}", vdf.len())
}

/// `appid` and `installdir` of an `appmanifest_*.acf`; `None` when either is
/// missing or the id is not a number.
pub fn parse_app_manifest(acf: &str) -> Option<SteamManifest> {
    todo!("{}", acf.len())
}

/// Finds Steam libraries from the Steam install path (when known).
#[derive(Debug, Clone)]
pub struct SteamLibraries {
    steam_path: Option<PathBuf>,
}

impl SteamLibraries {
    pub fn new(steam_path: Option<PathBuf>) -> Self {
        Self { steam_path }
    }
}

impl SteamLocator for SteamLibraries {
    fn library_folders(&self) -> Result<Vec<PathBuf>, AppError> {
        todo!("{:?}", self.steam_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_library_folders_unescapes_backslashes() {
        let vdf = r#""libraryfolders"
{
    "0"
    {
        "path"		"C:\\Program Files (x86)\\Steam"
        "label"		""
        "apps"
        {
            "230410"		"1234"
        }
    }
    "1"
    {
        "path"		"D:\\SteamLibrary"
    }
}"#;
        assert_eq!(
            parse_library_folders(vdf),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn parse_library_folders_supports_the_legacy_format() {
        let vdf = r#""LibraryFolders"
{
    "TimeNextStatsReport"		"123"
    "ContentStatsID"		"456"
    "1"		"E:\\Games\\Steam"
}"#;
        assert_eq!(
            parse_library_folders(vdf),
            vec![PathBuf::from(r"E:\Games\Steam")]
        );
    }

    #[test]
    fn parse_library_folders_of_garbage_is_empty() {
        assert!(parse_library_folders("").is_empty());
        assert!(parse_library_folders("\"unterminated").is_empty());
    }

    #[test]
    fn parse_app_manifest_reads_id_and_installdir() {
        let acf = r#""AppState"
{
    "appid"		"123"
    "name"		"MotoGP 25"
    "installdir"		"MotoGP25"
}"#;
        assert_eq!(
            parse_app_manifest(acf),
            Some(SteamManifest {
                app_id: 123,
                install_dir: "MotoGP25".into()
            })
        );
    }

    #[test]
    fn parse_app_manifest_rejects_incomplete_or_invalid_input() {
        assert_eq!(parse_app_manifest(r#""AppState" { "appid" "12" }"#), None);
        assert_eq!(
            parse_app_manifest(r#""AppState" { "appid" "abc" "installdir" "X" }"#),
            None
        );
        assert_eq!(parse_app_manifest(""), None);
    }

    #[test]
    fn steam_libraries_without_steam_path_is_empty() {
        assert_eq!(SteamLibraries::new(None).library_folders(), Ok(vec![]));
    }

    #[test]
    fn steam_libraries_includes_install_path_and_listed_libraries() {
        let tmp = tempfile::tempdir().unwrap();
        let steamapps = tmp.path().join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        let vdf = format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"D:\\\\Lib\" }} }}",
            tmp.path().to_string_lossy().replace('\\', "\\\\")
        );
        std::fs::write(steamapps.join("libraryfolders.vdf"), vdf).unwrap();
        let libs = SteamLibraries::new(Some(tmp.path().to_path_buf()))
            .library_folders()
            .unwrap();
        assert_eq!(
            libs,
            vec![tmp.path().to_path_buf(), PathBuf::from(r"D:\Lib")]
        );
    }

    #[test]
    fn steam_libraries_without_vdf_returns_only_the_install_path() {
        let tmp = tempfile::tempdir().unwrap();
        let libs = SteamLibraries::new(Some(tmp.path().to_path_buf()))
            .library_folders()
            .unwrap();
        assert_eq!(libs, vec![tmp.path().to_path_buf()]);
    }
}
