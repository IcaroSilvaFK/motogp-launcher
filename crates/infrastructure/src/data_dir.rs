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
        todo!("red")
    }

    pub fn config_file(&self) -> PathBuf {
        todo!("red")
    }

    pub fn profiles_file(&self) -> PathBuf {
        todo!("red")
    }

    pub fn library_file(&self) -> PathBuf {
        todo!("red")
    }

    pub fn deployed_file(&self) -> PathBuf {
        todo!("red")
    }

    pub fn library_dir(&self) -> PathBuf {
        todo!("red")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_well_known_paths_under_the_root() {
        let data = DataDir::new("C:/Usuário Padrão/data");
        let root = Path::new("C:/Usuário Padrão/data");
        assert_eq!(data.root(), root);
        assert_eq!(data.config_file(), root.join("config.json"));
        assert_eq!(data.profiles_file(), root.join("profiles.json"));
        assert_eq!(data.library_file(), root.join("library.json"));
        assert_eq!(data.deployed_file(), root.join("deployed.json"));
        assert_eq!(data.library_dir(), root.join("library"));
    }
}
