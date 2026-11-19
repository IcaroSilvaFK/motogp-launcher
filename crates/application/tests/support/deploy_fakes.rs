//! In-memory fakes of the deploy ports with injectable failures and a
//! simulated "process death" (every later operation fails until revived).
#![allow(dead_code)]
use motogp_application::{
    AppError, DeployError, DeployFiles, DeployMod, DeployService, DeployedRepository, DirCheck,
};
use motogp_domain::Deployed;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Default)]
pub struct DeployState {
    /// Files on disk: (directory, name) -> content.
    pub disk: BTreeMap<(PathBuf, String), Vec<u8>>,
    pub dirs: Vec<PathBuf>,
    pub manifest: Deployed,
    pub manifest_saves: usize,
    pub manifest_fails_to_load: bool,
    pub materialize_calls: usize,
    /// The process dies right before / right after the n-th (0-based) materialization.
    pub crash_before_materialize: Option<usize>,
    pub crash_after_materialize: Option<usize>,
    /// The n-th materialization fails with an I/O error (the process survives).
    pub fail_materialize: Option<usize>,
    /// Names whose removal fails.
    pub fail_remove: Vec<String>,
    pub dead: bool,
    /// A foreign (user) file appears at `name` right before the n-th materialization.
    pub user_file_before_materialize: Option<(usize, String, Vec<u8>)>,
    /// The n-th (0-based) manifest save fails once (the process survives).
    pub fail_save_at: Option<usize>,
    /// Ordered log of "materialize <name>" / "remove <name>".
    pub log: Vec<String>,
    /// Source file contents by path.
    pub sources: BTreeMap<PathBuf, Vec<u8>>,
}

pub type Shared = Rc<RefCell<DeployState>>;

pub fn new_state() -> Shared {
    Rc::new(RefCell::new(DeployState::default()))
}

pub fn mods_dir() -> PathBuf {
    PathBuf::from("game/~mods")
}

pub fn io_error(detail: &str) -> DeployError {
    DeployError::Io {
        location: "fake".into(),
        detail: detail.into(),
    }
}

pub struct FakeRepo(pub Shared);
pub struct FakeFiles(pub Shared);

impl DeployedRepository for FakeRepo {
    fn load(&self) -> Result<Deployed, DeployError> {
        let state = self.0.borrow();
        if state.dead || state.manifest_fails_to_load {
            return Err(DeployError::Storage(AppError::Read {
                location: "deployed.json".into(),
                detail: "unreadable".into(),
            }));
        }
        Ok(state.manifest.clone())
    }

    fn save(&self, deployed: &Deployed) -> Result<(), DeployError> {
        let mut state = self.0.borrow_mut();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        let call = state.manifest_saves;
        state.manifest_saves += 1;
        if state.fail_save_at == Some(call) {
            return Err(io_error("manifest save failed"));
        }
        state.manifest = deployed.clone();
        Ok(())
    }
}

impl DeployFiles for FakeFiles {
    fn exists(&self, dir: &Path, name: &str) -> Result<bool, DeployError> {
        let state = self.0.borrow();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        Ok(state
            .disk
            .contains_key(&(dir.to_path_buf(), name.to_string())))
    }

    fn check_dir(&self, manifest_dir: &Path, authorized: &Path) -> Result<DirCheck, DeployError> {
        let state = self.0.borrow();
        if manifest_dir != authorized {
            return Ok(DirCheck::Foreign);
        }
        let present = state.dirs.iter().any(|d| d == manifest_dir)
            || state.disk.keys().any(|(d, _)| d == manifest_dir);
        Ok(if present {
            DirCheck::Authorized
        } else {
            DirCheck::Missing
        })
    }

    /// Fakes have no inode identity: a file is ours when its bytes equal the
    /// library source (what a hard link or a faithful copy looks like).
    fn owns(&self, dir: &Path, name: &str, source: &Path) -> Result<bool, DeployError> {
        let state = self.0.borrow();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        Ok(
            match (
                state.disk.get(&(dir.to_path_buf(), name.to_string())),
                state.sources.get(source),
            ) {
                (Some(on_disk), Some(original)) => on_disk == original,
                _ => false,
            },
        )
    }

    fn create_dir_all(&self, dir: &Path) -> Result<(), DeployError> {
        let mut state = self.0.borrow_mut();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        if !state.dirs.iter().any(|d| d == dir) {
            state.dirs.push(dir.to_path_buf());
        }
        Ok(())
    }

    fn materialize(&self, source: &Path, dir: &Path, name: &str) -> Result<(), DeployError> {
        let mut state = self.0.borrow_mut();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        let call = state.materialize_calls;
        state.materialize_calls += 1;
        if state.crash_before_materialize == Some(call) {
            state.dead = true;
            return Err(io_error("crash before materialization"));
        }
        if state.fail_materialize == Some(call) {
            return Err(io_error("disk full"));
        }
        if let Some((at, user_name, content)) = state.user_file_before_materialize.clone() {
            if at == call {
                state.disk.insert((dir.to_path_buf(), user_name), content);
            }
        }
        let key = (dir.to_path_buf(), name.to_string());
        if state.disk.contains_key(&key) {
            return Err(DeployError::AlreadyExists { name: name.into() });
        }
        let content = state.sources.get(source).cloned().unwrap_or_default();
        state.disk.insert(key, content);
        state.log.push(format!("materialize {name}"));
        if state.crash_after_materialize == Some(call) {
            state.dead = true;
            return Err(io_error("crash after materialization"));
        }
        Ok(())
    }

    fn remove(&self, dir: &Path, name: &str) -> Result<bool, DeployError> {
        let mut state = self.0.borrow_mut();
        if state.dead {
            return Err(io_error("process is dead"));
        }
        if state.fail_remove.iter().any(|n| n == name) {
            return Err(io_error("access denied"));
        }
        state.log.push(format!("remove {name}"));
        Ok(state
            .disk
            .remove(&(dir.to_path_buf(), name.to_string()))
            .is_some())
    }
}

pub fn manifest_names(files: &[motogp_domain::DeployedFile]) -> Vec<String> {
    files.iter().map(|f| f.name.clone()).collect()
}

pub type FakeDeployService = DeployService<FakeRepo, FakeFiles>;

pub fn service(state: &Shared) -> FakeDeployService {
    DeployService::new(FakeRepo(state.clone()), FakeFiles(state.clone()))
}

/// Registers a library source file `lib/<id>/<name>`.
pub fn put_source(state: &Shared, id: &str, name: &str, content: &[u8]) -> PathBuf {
    let path = PathBuf::from("lib").join(id).join(name);
    state
        .borrow_mut()
        .sources
        .insert(path.clone(), content.to_vec());
    path
}

pub fn put_disk(state: &Shared, name: &str, content: &[u8]) {
    state
        .borrow_mut()
        .disk
        .insert((mods_dir(), name.to_string()), content.to_vec());
}

pub fn disk_names(state: &Shared) -> Vec<String> {
    state
        .borrow()
        .disk
        .keys()
        .map(|(_, name)| name.clone())
        .collect()
}

pub fn disk_has(state: &Shared, name: &str) -> bool {
    state
        .borrow()
        .disk
        .contains_key(&(mods_dir(), name.to_string()))
}

pub fn mod_of(id: &str, files: &[&str]) -> DeployMod {
    DeployMod {
        id: id.into(),
        source_dir: PathBuf::from("lib").join(id),
        files: files.iter().map(|f| f.to_string()).collect(),
    }
}

pub const ID_A: &str = "aaaaaaaaaaaa";
pub const ID_B: &str = "bbbbbbbbbbbb";

/// Registers sources for the given mods (content = file name bytes).
pub fn put_sources(state: &Shared, mods: &[DeployMod]) {
    for m in mods {
        for f in &m.files {
            put_source(state, &m.id, f, f.as_bytes());
        }
    }
}
