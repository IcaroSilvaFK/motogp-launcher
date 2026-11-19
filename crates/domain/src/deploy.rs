use std::path::PathBuf;

/// Manifest of the files deployed into the game `~mods` folder (the source for
/// session recovery). Pure data: no I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Deployed {
    /// Directory the files were deployed into.
    pub mods_dir: PathBuf,
    /// File names (never paths) proven deployed by this launcher.
    pub files: Vec<String>,
    /// Journal: names recorded *before* their materialization. An entry here
    /// was absent from `mods_dir` when it was recorded, so the launcher owns
    /// whatever exists under that name; recovery treats it like `files`.
    pub pending: Vec<String>,
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
            .map(String::as_str)
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

#[cfg(test)]
mod tests {
    use super::*;

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
            files: vec!["a".into()],
            pending: vec!["b".into()],
        };
        assert!(!deployed.is_empty());
        assert_eq!(deployed.tracked().collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
