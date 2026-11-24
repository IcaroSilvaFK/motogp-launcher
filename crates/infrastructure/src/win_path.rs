//! Pure normalization of Windows drive-absolute paths for display and storage.
//!
//! Steam writes `SteamPath` lower-cased with forward slashes, which later gets
//! joined with backslashes into a mixed-separator path. This helper only touches
//! strings, so it is safe (and a no-op for non-drive paths) on every platform.
use std::path::{Path, PathBuf};

/// Normalizes a drive-absolute Windows path: `/` becomes `\`, repeated
/// separators collapse, a trailing separator is dropped (except for `C:\`) and
/// the drive letter is upper-cased. Anything else (relative, UNC, `\\?\`
/// verbatim, Unix, non-UTF-8) is returned unchanged. The case of everything
/// after the drive letter and all non-ASCII characters are preserved.
pub fn normalize_windows_path(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    let bytes = text.as_bytes();
    let drive_absolute = bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes.get(2), None | Some(b'/') | Some(b'\\'));
    if !drive_absolute {
        return path.to_path_buf();
    }
    let drive = (bytes[0] as char).to_ascii_uppercase();
    let rest = text[2..]
        .split(['/', '\\'])
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("\\");
    PathBuf::from(format!("{drive}:\\{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> PathBuf {
        normalize_windows_path(Path::new(s))
    }

    #[test]
    fn registry_style_path_is_normalized() {
        assert_eq!(
            norm("c:/program files (x86)/steam"),
            PathBuf::from(r"C:\program files (x86)\steam")
        );
    }

    #[test]
    fn mixed_separators_with_accents_and_trademark_survive() {
        // "Usuário Padrão" is a real-world accented Windows folder name; kept on purpose.
        assert_eq!(
            norm(r"c:/Users/Usuário Padrão/Steam\steamapps\common\MotoGP™26"),
            PathBuf::from(r"C:\Users\Usuário Padrão\Steam\steamapps\common\MotoGP™26")
        );
    }

    #[test]
    fn relative_unc_verbatim_and_unix_paths_are_unchanged() {
        for s in [
            "relative/dir",
            r"Program Files (x86)",
            r"\\server\share/x",
            r"\\?\C:\x",
            "/usr/local/steam",
            "C:relative",
            "",
        ] {
            assert_eq!(norm(s), PathBuf::from(s), "{s}");
        }
    }

    #[test]
    fn drive_roots_and_repeated_separators_are_cleaned() {
        assert_eq!(norm("C:/"), PathBuf::from(r"C:\"));
        assert_eq!(norm("c:"), PathBuf::from(r"C:\"));
        assert_eq!(norm(r"c:\\a//b/"), PathBuf::from(r"C:\a\b"));
    }
}
