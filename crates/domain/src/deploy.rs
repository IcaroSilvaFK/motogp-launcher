use std::path::{Path, PathBuf};

/// One file the launcher deployed (or intends to deploy) into `~mods`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployedFile {
    /// Destination file name (never a path).
    pub name: String,
    /// Library file it was created from. Ownership proof: the destination is
    /// ours only while it is that same file (hard link) or a byte-for-byte copy.
    pub source: PathBuf,
}

impl DeployedFile {
    pub fn new(name: impl Into<String>, source: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
        }
    }
}

/// Manifest of the files deployed into the game `~mods` folder (the source for
/// session recovery). Pure data: no I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deployed {
    /// Directory the files were deployed into (never trusted on its own: the
    /// application checks it against the authorized `~mods` before removing).
    pub mods_dir: PathBuf,
    /// Files the launcher created.
    pub files: Vec<DeployedFile>,
    /// Journal: files recorded *before* their materialization. A journal entry
    /// proves only an intent, not ownership: a file under that name is deleted
    /// only if it is verified to be the launcher's own (see `DeployedFile::source`).
    pub pending: Vec<DeployedFile>,
}

impl Deployed {
    /// `true` when nothing is tracked.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.pending.is_empty()
    }

    /// Every tracked name: deployed files first, then journal entries.
    pub fn tracked(&self) -> impl Iterator<Item = &str> {
        self.files
            .iter()
            .chain(self.pending.iter())
            .map(|f| f.name.as_str())
    }
}

/// Destination name `NNN_<modId>_<original>`; `index` is the position of the
/// mod in the profile (from zero), shared by all files of that mod. The
/// original name, including a `_P` before the extension, is kept intact.
pub fn deployed_name(index: usize, mod_id: &str, original: &str) -> String {
    format!("{index:03}_{mod_id}_{original}")
}

/// `true` for a simple relative file name: no separators, no drive prefix, no
/// `.`/`..`, no NUL and no trailing dot or space (which Windows would trim).
pub fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !name.contains(['/', '\\', ':', '\0'])
}

/// `true` when the last component of `path` is `~mods`, compared
/// case-insensitively (Windows). A defense-in-depth check that a folder about
/// to be cleaned really is a game `~mods` folder. `""`, `/`, `..` and names
/// such as `~mods.bak` are not.
pub fn is_mods_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("~mods"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_dir_is_recognized_by_its_last_component() {
        for path in [
            "~mods",
            "~MODS",
            "game/Content/Paks/~mods",
            "game/Content/Paks/~mods/",
        ] {
            assert!(is_mods_dir(Path::new(path)), "{path:?} should match");
        }
        #[cfg(windows)]
        assert!(is_mods_dir(Path::new("C:\\Game\\Content\\Paks\\~mods\\")));
    }

    #[test]
    fn other_directories_are_not_mods_dirs() {
        for path in [
            "",
            "/",
            "..",
            "game/~mods/..",
            "game/~mods.bak",
            "game/mods",
            "~mods/sub",
        ] {
            assert!(!is_mods_dir(Path::new(path)), "{path:?} should not match");
        }
    }

    #[test]
    fn deploy_preserves_p_suffix() {
        let name = deployed_name(0, "a1b2c3d4e5f6", "Ducati_P.pak");
        assert_eq!(name, "000_a1b2c3d4e5f6_Ducati_P.pak");
        assert!(name.ends_with("_Ducati_P.pak"));
    }

    #[test]
    fn index_is_zero_padded_to_three_digits_and_grows_beyond() {
        assert_eq!(deployed_name(7, "a", "x.pak"), "007_a_x.pak");
        assert_eq!(deployed_name(1234, "a", "x.pak"), "1234_a_x.pak");
    }

    #[test]
    fn safe_names_are_simple_relative_file_names() {
        for name in ["A.pak", "000_a_B_P.pak", "with space.pak", "a.b.c"] {
            assert!(is_safe_file_name(name), "{name:?} should be safe");
        }
    }

    #[test]
    fn unsafe_names_are_rejected() {
        for name in [
            "",
            ".",
            "..",
            "../a.pak",
            "..\\a.pak",
            "a/b.pak",
            "a\\b.pak",
            "/abs.pak",
            "C:\\abs.pak",
            "C:abs.pak",
            "a\0b",
            "trailing.",
            "trailing ",
        ] {
            assert!(!is_safe_file_name(name), "{name:?} should be unsafe");
        }
    }

    #[test]
    fn default_manifest_is_empty() {
        let deployed = Deployed::default();
        assert!(deployed.is_empty());
        assert_eq!(deployed.tracked().count(), 0);
    }

    #[test]
    fn tracked_lists_deployed_and_pending_files() {
        let deployed = Deployed {
            mods_dir: "mods".into(),
            files: vec![DeployedFile::new("a", "lib/a")],
            pending: vec![DeployedFile::new("b", "lib/b")],
        };
        assert!(!deployed.is_empty());
        assert_eq!(deployed.tracked().collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
