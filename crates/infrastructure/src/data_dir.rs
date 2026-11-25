use std::path::{Path, PathBuf};

/// Root of the application data and the well-known paths below it.
/// Resolving the root itself (Tauri `app_data_dir`) belongs to the interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn profiles_file(&self) -> PathBuf {
        self.root.join("profiles.json")
    }

    pub fn library_file(&self) -> PathBuf {
        self.root.join("library.json")
    }

    pub fn deployed_file(&self) -> PathBuf {
        self.root.join("deployed.json")
    }

    pub fn catalog_cache_file(&self) -> PathBuf {
        self.root.join("catalog-cache.json")
    }

    pub fn library_dir(&self) -> PathBuf {
        self.root.join("library")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_well_known_paths_under_the_root() {
        // Non-ASCII on purpose: Windows user folders often contain accents (e.g. "Usuário Padrão").
        let data = DataDir::new("C:/Usuário Padrão/data");
        let root = Path::new("C:/Usuário Padrão/data");
        assert_eq!(data.root(), root);
        assert_eq!(data.config_file(), root.join("config.json"));
        assert_eq!(data.profiles_file(), root.join("profiles.json"));
        assert_eq!(data.library_file(), root.join("library.json"));
        assert_eq!(data.deployed_file(), root.join("deployed.json"));
        assert_eq!(data.library_dir(), root.join("library"));
        assert_eq!(data.catalog_cache_file(), root.join("catalog-cache.json"));
    }
}
