use std::collections::BTreeMap;
use std::fmt;

/// Extensions that belong to a mod (compared case-insensitively).
pub const MOD_EXTENSIONS: [&str; 3] = ["pak", "utoc", "ucas"];

/// A set of sibling files sharing one base name, with at least one `.pak`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModGroup {
    /// Base name (file name without extension), suffix `_P` preserved.
    pub base_name: String,
    /// File names of the group, sorted.
    pub files: Vec<String>,
}

impl ModGroup {
    /// The `.pak` file of the group.
    pub fn pak_file(&self) -> Option<&str> {
        self.files
            .iter()
            .map(String::as_str)
            .find(|name| extension_of(name).as_deref() == Some("pak"))
    }

    /// Display name: the base name without the final `_P`.
    pub fn display_name(&self) -> String {
        display_name(&self.base_name)
    }
}

/// A group violates the rule that every mod needs a `.pak`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupingError {
    MissingPak { group: String },
}

impl fmt::Display for GroupingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupingError::MissingPak { group } => {
                write!(f, "mod \"{group}\" has no .pak file")
            }
        }
    }
}

impl std::error::Error for GroupingError {}

/// Position of the extension dot; a leading or trailing dot is not an extension.
fn extension_dot(file_name: &str) -> Option<usize> {
    file_name
        .rfind('.')
        .filter(|&i| i > 0 && i + 1 < file_name.len())
}

/// Lowercased extension of a file name, when it has one.
pub fn extension_of(file_name: &str) -> Option<String> {
    extension_dot(file_name).map(|i| file_name[i + 1..].to_ascii_lowercase())
}

/// File name without its extension.
pub fn base_name(file_name: &str) -> &str {
    extension_dot(file_name).map_or(file_name, |i| &file_name[..i])
}

/// `true` when the file name has a mod extension (`.pak`, `.utoc`, `.ucas`).
pub fn is_mod_file(file_name: &str) -> bool {
    extension_of(file_name).is_some_and(|ext| MOD_EXTENSIONS.contains(&ext.as_str()))
}

/// Removes only a final `_P` from a base name.
pub fn display_name(base_name: &str) -> String {
    base_name
        .strip_suffix("_P")
        .filter(|name| !name.is_empty())
        .unwrap_or(base_name)
        .to_string()
}

/// Groups file names by base name. Unrelated extensions are ignored and every
/// resulting group must contain a `.pak`.
pub fn group_pak_files(file_names: &[String]) -> Result<Vec<ModGroup>, GroupingError> {
    let mut groups: BTreeMap<String, ModGroup> = BTreeMap::new();
    for name in file_names.iter().filter(|name| is_mod_file(name)) {
        let base = base_name(name);
        groups
            .entry(base.to_lowercase())
            .or_insert_with(|| ModGroup {
                base_name: base.to_string(),
                files: Vec::new(),
            })
            .files
            .push(name.clone());
    }
    groups
        .into_values()
        .map(|mut group| {
            group.files.sort();
            group.files.dedup();
            match group.pak_file() {
                Some(_) => Ok(group),
                None => Err(GroupingError::MissingPak {
                    group: group.base_name,
                }),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn utoc_without_pak_errors() {
        let error = group_pak_files(&names(&["A.utoc"])).unwrap_err();
        assert_eq!(
            error,
            GroupingError::MissingPak {
                group: "A".to_string()
            }
        );
        assert!(error.to_string().contains("has no .pak file"));
    }

    #[test]
    fn group_accepts_case_insensitive_extensions() {
        let groups = group_pak_files(&names(&["A.PAK", "A.Utoc", "a.UCAS"])).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 3);
        assert_eq!(groups[0].pak_file(), Some("A.PAK"));
    }

    #[test]
    fn group_ignores_unrelated_extensions() {
        let groups =
            group_pak_files(&names(&["A.pak", "readme.txt", "A.sig", "noext", ".pak"])).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files, names(&["A.pak"]));
    }

    #[test]
    fn groups_by_base_name_and_keeps_suffix_in_file_names() {
        let groups = group_pak_files(&names(&[
            "B.pak", "A_P.utoc", "A_P.pak", "A_P.ucas", "A_P.pak",
        ]))
        .unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].base_name, "A_P");
        assert_eq!(groups[0].display_name(), "A");
        assert_eq!(groups[0].files, names(&["A_P.pak", "A_P.ucas", "A_P.utoc"]));
        assert_eq!(groups[1].base_name, "B");
    }

    #[test]
    fn no_files_means_no_groups() {
        assert_eq!(group_pak_files(&[]), Ok(vec![]));
        assert_eq!(group_pak_files(&names(&["readme.txt"])), Ok(vec![]));
    }

    #[test]
    fn display_name_strips_only_the_final_p_suffix() {
        assert_eq!(display_name("A_P"), "A");
        assert_eq!(display_name("A_P_P"), "A_P");
        assert_eq!(display_name("A_Pro"), "A_Pro");
        assert_eq!(display_name("A"), "A");
        assert_eq!(display_name("_P"), "_P");
    }

    #[test]
    fn extension_and_base_name_helpers() {
        assert_eq!(extension_of("A.PAK"), Some("pak".to_string()));
        assert_eq!(extension_of("noext"), None);
        assert_eq!(extension_of(".pak"), None);
        assert_eq!(extension_of("trailing."), None);
        assert_eq!(base_name("A.tar.pak"), "A.tar");
        assert_eq!(base_name("noext"), "noext");
        assert!(is_mod_file("A.Ucas"));
        assert!(!is_mod_file("A.zip"));
    }

    #[test]
    fn pak_file_is_absent_when_the_group_has_none() {
        let group = ModGroup {
            base_name: "A".into(),
            files: names(&["A.utoc"]),
        };
        assert_eq!(group.pak_file(), None);
    }
}
