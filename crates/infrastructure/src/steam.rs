//! Steam file formats (VDF/ACF) and the Steam library locator adapter.
use motogp_application::{AppError, SteamLocator};
use motogp_domain::SteamManifest;
use std::path::{Path, PathBuf};

/// Library roots listed in `libraryfolders.vdf`, with VDF escapes resolved
/// (`\\` becomes `\`). Handles both the modern (`"path"`) and legacy formats.
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let tokens = tokenize(vdf);
    let mut libraries = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        match (&tokens[i], tokens.get(i + 1)) {
            (Token::Str(key), Some(Token::Str(value))) => {
                let legacy_entry = depth == 1 && key.chars().all(|c| c.is_ascii_digit());
                if key.eq_ignore_ascii_case("path") || legacy_entry {
                    libraries.push(PathBuf::from(value));
                }
                i += 2;
            }
            (Token::Open, _) => {
                depth += 1;
                i += 1;
            }
            (Token::Close, _) => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => i += 1,
        }
    }
    libraries
}

/// `appid` and `installdir` of an `appmanifest_*.acf`; `None` when either is
/// missing or the id is not a number.
pub fn parse_app_manifest(acf: &str) -> Option<SteamManifest> {
    let tokens = tokenize(acf);
    let mut app_id: Option<u32> = None;
    let mut install_dir: Option<String> = None;
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        match (&tokens[i], tokens.get(i + 1)) {
            // Non-overlapping key/value pair; only the `AppState` level counts.
            (Token::Str(key), Some(Token::Str(value))) => {
                if depth == 1 {
                    if key.eq_ignore_ascii_case("appid") && app_id.is_none() {
                        app_id = value.trim().parse().ok();
                    } else if key.eq_ignore_ascii_case("installdir") && install_dir.is_none() {
                        install_dir = Some(value.clone());
                    }
                }
                i += 2;
            }
            (Token::Open, _) => {
                depth += 1;
                i += 1;
            }
            (Token::Close, _) => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => i += 1,
        }
    }
    Some(SteamManifest {
        app_id: app_id?,
        install_dir: install_dir?,
    })
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Str(String),
    Open,
    Close,
}

/// Minimal VDF tokenizer: quoted strings (with backslash escapes) and braces.
/// An unterminated string ends tokenization.
fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // `//` comment outside a string: skip to the end of the line.
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        break;
                    }
                }
            }
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            '"' => {
                let mut value = String::new();
                loop {
                    match chars.next() {
                        None => return tokens,
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => value.push('\n'),
                            Some('t') => value.push('\t'),
                            Some(other) => value.push(other),
                            None => return tokens,
                        },
                        Some(other) => value.push(other),
                    }
                }
                tokens.push(Token::Str(value));
            }
            _ => {}
        }
    }
    tokens
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
        let Some(steam_path) = &self.steam_path else {
            return Ok(Vec::new());
        };
        let vdf_path = steam_path.join("steamapps").join("libraryfolders.vdf");
        let listed = match std::fs::read_to_string(&vdf_path) {
            Ok(text) => parse_library_folders(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                return Err(AppError::Read {
                    location: vdf_path.display().to_string(),
                    detail: e.to_string(),
                })
            }
        };
        let mut libraries: Vec<PathBuf> = vec![steam_path.clone()];
        for library in listed {
            if !libraries.iter().any(|l| same_path(l, &library)) {
                libraries.push(library);
            }
        }
        Ok(libraries)
    }
}

/// Registry paths use forward slashes and VDF paths backslashes; compare
/// ignoring case and separators.
fn same_path(a: &Path, b: &Path) -> bool {
    let key = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    };
    key(a) == key(b)
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
    fn parse_app_manifest_ignores_overlapping_pairs() {
        let acf = r#""AppState" { "appid" "123" "name" "installdir" "installdir" "MotoGP25" }"#;
        assert_eq!(
            parse_app_manifest(acf),
            Some(SteamManifest {
                app_id: 123,
                install_dir: "MotoGP25".into()
            })
        );
    }

    #[test]
    fn parse_app_manifest_skips_comments() {
        let acf = "\"AppState\"\r\n{\r\n// \"appid\" \"999\"\r\n\"appid\" \"123\"\r\n\"installdir\" \"MotoGP25\" // \"installdir\" \"Wrong\"\r\n}";
        assert_eq!(
            parse_app_manifest(acf),
            Some(SteamManifest {
                app_id: 123,
                install_dir: "MotoGP25".into()
            })
        );
    }

    #[test]
    fn parse_library_folders_skips_comments() {
        let vdf = "\"libraryfolders\"\r\n{\r\n\"0\"\r\n{\r\n// \"path\" \"C:\\\\Commented\"\r\n\"path\" \"D:\\\\Real\" // { \"path\" \"E:\\\\Nope\"\r\n}\r\n}";
        assert_eq!(parse_library_folders(vdf), vec![PathBuf::from(r"D:\Real")]);
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
