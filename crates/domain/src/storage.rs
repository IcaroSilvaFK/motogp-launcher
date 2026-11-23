//! Storage usage of the launcher data folder: how an entry of the folder is
//! classified and how the per-category sizes add up. Pure rules, no I/O.
use std::path::PathBuf;

/// Name of the folder that holds the imported mods.
const MODS_DIR: &str = "library";
/// State files the launcher keeps in the data folder root.
const STATE_FILES: [&str; 4] = [
    "config.json",
    "profiles.json",
    "library.json",
    "deployed.json",
];

/// What a part of the data folder is used for, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageCategory {
    /// The imported mods (`library/`).
    Mods,
    /// Configuration, profiles, library index and deploy manifest.
    LauncherData,
    /// Anything else found in the data folder.
    Other,
}

impl StorageCategory {
    /// Every category, in the fixed display order.
    pub const ALL: [StorageCategory; 3] = [
        StorageCategory::Mods,
        StorageCategory::LauncherData,
        StorageCategory::Other,
    ];
}

/// Classifies a direct child of the data folder. Matching is ASCII
/// case-insensitive, like the Windows file system.
pub fn classify_entry(name: &str, is_dir: bool) -> StorageCategory {
    if is_dir {
        return if name.eq_ignore_ascii_case(MODS_DIR) {
            StorageCategory::Mods
        } else {
            StorageCategory::Other
        };
    }
    let base = strip_tmp(name);
    if STATE_FILES.iter().any(|f| base.eq_ignore_ascii_case(f)) {
        StorageCategory::LauncherData
    } else {
        StorageCategory::Other
    }
}

/// `x.json.tmp` (an atomic-write sibling) counts like `x.json`.
fn strip_tmp(name: &str) -> &str {
    let len = name.len();
    if len >= 4 && name.is_char_boundary(len - 4) && name[len - 4..].eq_ignore_ascii_case(".tmp") {
        &name[..len - 4]
    } else {
        name
    }
}

/// Measured usage of the data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageUsage {
    pub root: PathBuf,
    /// Every category exactly once, in the fixed display order.
    pub categories: Vec<(StorageCategory, u64)>,
    /// Free space of the drive holding the folder; `None` when unknown.
    pub free_bytes: Option<u64>,
}

impl StorageUsage {
    /// Groups the sizes of the entries by category (saturating) and keeps every
    /// category, even with 0 bytes.
    pub fn from_entries(
        root: PathBuf,
        entries: impl IntoIterator<Item = (StorageCategory, u64)>,
        free_bytes: Option<u64>,
    ) -> Self {
        let mut categories: Vec<(StorageCategory, u64)> =
            StorageCategory::ALL.iter().map(|c| (*c, 0)).collect();
        for (category, bytes) in entries {
            if let Some(slot) = categories.iter_mut().find(|(c, _)| *c == category) {
                slot.1 = slot.1.saturating_add(bytes);
            }
        }
        Self {
            root,
            categories,
            free_bytes,
        }
    }

    /// Sum of the categories (saturating).
    pub fn total_bytes(&self) -> u64 {
        self.categories
            .iter()
            .fold(0u64, |sum, (_, bytes)| sum.saturating_add(*bytes))
    }

    /// Bytes of one category.
    pub fn bytes_of(&self, category: StorageCategory) -> u64 {
        self.categories
            .iter()
            .find(|(c, _)| *c == category)
            .map_or(0, |(_, bytes)| *bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_directory_is_mods_in_any_case() {
        assert_eq!(classify_entry("library", true), StorageCategory::Mods);
        assert_eq!(classify_entry("Library", true), StorageCategory::Mods);
    }

    #[test]
    fn a_library_file_is_other() {
        assert_eq!(classify_entry("library", false), StorageCategory::Other);
    }

    #[test]
    fn state_files_and_their_tmp_siblings_are_launcher_data() {
        for name in [
            "config.json",
            "PROFILES.JSON",
            "library.json",
            "deployed.json",
            "deployed.json.tmp",
            "Config.JSON.TMP",
        ] {
            assert_eq!(
                classify_entry(name, false),
                StorageCategory::LauncherData,
                "{name}"
            );
        }
    }

    #[test]
    fn other_entries_are_other() {
        for (name, is_dir) in [
            ("notes.txt", false),
            ("crash.dmp", false),
            (".tmp", false),
            ("config.json", true),
            ("logs", true),
            // Non-ASCII on purpose: Windows user folders often contain accents.
            ("Usuário Padrão.tmp", false),
        ] {
            assert_eq!(classify_entry(name, is_dir), StorageCategory::Other);
        }
    }

    #[test]
    fn usage_keeps_every_category_in_order_even_when_empty() {
        let usage = StorageUsage::from_entries(PathBuf::from("root"), [], None);
        assert_eq!(
            usage.categories,
            vec![
                (StorageCategory::Mods, 0),
                (StorageCategory::LauncherData, 0),
                (StorageCategory::Other, 0)
            ]
        );
        assert_eq!(usage.total_bytes(), 0);
    }

    #[test]
    fn usage_sums_entries_per_category() {
        let usage = StorageUsage::from_entries(
            PathBuf::from("root"),
            [
                (StorageCategory::Other, 5),
                (StorageCategory::Mods, 300),
                (StorageCategory::LauncherData, 12),
                (StorageCategory::LauncherData, 8),
            ],
            Some(9),
        );
        assert_eq!(usage.bytes_of(StorageCategory::Mods), 300);
        assert_eq!(usage.bytes_of(StorageCategory::LauncherData), 20);
        assert_eq!(usage.total_bytes(), 325);
        assert_eq!(usage.free_bytes, Some(9));
    }

    #[test]
    fn sums_saturate_instead_of_overflowing() {
        let usage = StorageUsage::from_entries(
            PathBuf::from("root"),
            [
                (StorageCategory::Mods, u64::MAX),
                (StorageCategory::Mods, 10),
                (StorageCategory::Other, 10),
            ],
            None,
        );
        assert_eq!(usage.bytes_of(StorageCategory::Mods), u64::MAX);
        assert_eq!(usage.total_bytes(), u64::MAX);
    }

    #[test]
    fn bytes_of_a_missing_category_is_zero() {
        let usage = StorageUsage {
            root: PathBuf::new(),
            categories: vec![],
            free_bytes: None,
        };
        assert_eq!(usage.bytes_of(StorageCategory::Mods), 0);
    }
}
