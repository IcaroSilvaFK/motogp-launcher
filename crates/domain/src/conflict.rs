//! File conflicts between mods of a load order (pure rules).
use crate::ModEntry;

/// Two or more mods of a load order shipping the same file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConflict {
    /// File name as spelled by the winning mod.
    pub path: String,
    /// Involved mod IDs, in load order (the last one wins).
    pub mod_ids: Vec<String>,
}

impl FileConflict {
    /// The mod that loads last, so its file takes effect.
    pub fn winner(&self) -> &str {
        self.mod_ids.last().map(String::as_str).unwrap_or_default()
    }
}

/// Detects file-name conflicts for `order` against `library`.
///
/// IDs missing from the library and repeated IDs are ignored. File names are
/// compared ASCII case-insensitively (Windows). Sorted by lowercased path.
pub fn detect_conflicts(order: &[String], library: &[ModEntry]) -> Vec<FileConflict> {
    // (lowercased name, spelling of the latest mod, mods in load order)
    let mut groups: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for id in order {
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id);
        let Some(entry) = library.iter().find(|entry| entry.id == *id) else {
            continue;
        };
        for file in &entry.files {
            let key = file.to_ascii_lowercase();
            match groups.iter_mut().find(|group| group.0 == key) {
                Some(group) => {
                    group.1 = file.clone();
                    if group.2.last() != Some(id) {
                        group.2.push(id.clone());
                    }
                }
                None => groups.push((key, file.clone(), vec![id.clone()])),
            }
        }
    }
    let mut conflicts: Vec<(String, FileConflict)> = groups
        .into_iter()
        .filter(|group| group.2.len() > 1)
        .map(|(key, path, mod_ids)| (key, FileConflict { path, mod_ids }))
        .collect();
    conflicts.sort_by(|a, b| a.0.cmp(&b.0));
    conflicts
        .into_iter()
        .map(|(_, conflict)| conflict)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, files: &[&str]) -> ModEntry {
        ModEntry {
            id: id.into(),
            name: id.into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            size_bytes: 0,
        }
    }

    fn order(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn library() -> Vec<ModEntry> {
        vec![entry("a", &["Ducati_P.pak"]), entry("b", &["ducati_p.pak"])]
    }

    #[test]
    fn later_mod_wins_and_path_uses_the_winner_spelling() {
        let found = detect_conflicts(&order(&["a", "b"]), &library());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "ducati_p.pak");
        assert_eq!(found[0].mod_ids, ["a", "b"]);
        assert_eq!(found[0].winner(), "b");
    }

    #[test]
    fn reordering_changes_the_winner() {
        let found = detect_conflicts(&order(&["b", "a"]), &library());
        assert_eq!(found[0].path, "Ducati_P.pak");
        assert_eq!(found[0].mod_ids, ["b", "a"]);
        assert_eq!(found[0].winner(), "a");
    }

    #[test]
    fn disjoint_files_do_not_conflict() {
        let lib = vec![entry("a", &["A_P.pak"]), entry("b", &["B_P.pak"])];
        assert!(detect_conflicts(&order(&["a", "b"]), &lib).is_empty());
    }

    #[test]
    fn missing_and_duplicate_ids_are_ignored() {
        let found = detect_conflicts(&order(&["a", "zzz", "a"]), &library());
        assert!(found.is_empty());
    }

    #[test]
    fn a_mod_shipping_the_name_twice_counts_once() {
        let lib = vec![entry("a", &["X.pak", "x.PAK"])];
        assert!(detect_conflicts(&order(&["a"]), &lib).is_empty());
    }

    #[test]
    fn conflicts_are_sorted_by_lowercased_path() {
        let lib = vec![
            entry("a", &["Z.pak", "b.pak"]),
            entry("b", &["z.pak", "B.pak"]),
        ];
        let found = detect_conflicts(&order(&["a", "b"]), &lib);
        let paths: Vec<&str> = found.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, ["B.pak", "z.pak"]);
    }

    #[test]
    fn three_mods_keep_load_order() {
        let lib = vec![
            entry("a", &["x.pak"]),
            entry("b", &["x.pak"]),
            entry("c", &["X.pak"]),
        ];
        let found = detect_conflicts(&order(&["c", "a", "b"]), &lib);
        assert_eq!(found[0].mod_ids, ["c", "a", "b"]);
        assert_eq!(found[0].winner(), "b");
    }

    #[test]
    fn empty_winner_for_empty_conflict() {
        let conflict = FileConflict {
            path: "x".into(),
            mod_ids: vec![],
        };
        assert_eq!(conflict.winner(), "");
    }
}
