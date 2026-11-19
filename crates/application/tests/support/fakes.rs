//! In-memory fakes of the library ports, shared by the use-case tests and the
//! BDD steps. No real fs, ZIP or hashing is involved.
#![allow(dead_code)]
use motogp_application::{
    AppError, ArchiveReader, ContentHasher, LibraryError, LibraryFiles, ModRepository, ModService,
};
use motogp_domain::ModEntry;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Default)]
pub struct State {
    pub files: BTreeMap<PathBuf, Vec<u8>>,
    pub archives: HashMap<PathBuf, Vec<(String, Vec<u8>)>>,
    pub index: Vec<ModEntry>,
    pub staging_counter: u32,
    pub fail_copy: bool,
    pub fail_index_add: bool,
    pub fail_remove_dir: bool,
    pub fail_publish_id: Option<String>,
    pub discarded: Vec<PathBuf>,
}

pub type Shared = Rc<RefCell<State>>;

pub fn library_dir() -> PathBuf {
    PathBuf::from("library")
}

pub fn new_state() -> Shared {
    Rc::new(RefCell::new(State::default()))
}

fn io_error(location: &Path, detail: &str) -> LibraryError {
    LibraryError::Storage(AppError::Write {
        location: location.display().to_string(),
        detail: detail.to_string(),
    })
}

/// Fake hash: the content as hex, zero padded to 64 characters. The mod ID
/// (12 characters) is therefore the hex of the first six content bytes.
pub fn fake_hash(content: &[u8]) -> String {
    let mut hex: String = content.iter().map(|b| format!("{b:02x}")).collect();
    while hex.len() < 64 {
        hex.push('0');
    }
    hex
}

pub fn id_of(content: &[u8]) -> String {
    fake_hash(content)[..12].to_string()
}

pub fn put_file(state: &Shared, path: &str, content: &[u8]) {
    state
        .borrow_mut()
        .files
        .insert(PathBuf::from(path), content.to_vec());
}

pub fn put_archive(state: &Shared, path: &str, entries: &[(&str, &[u8])]) {
    state.borrow_mut().archives.insert(
        PathBuf::from(path),
        entries
            .iter()
            .map(|(n, c)| (n.to_string(), c.to_vec()))
            .collect(),
    );
}

pub struct FakeRepo(pub Shared);
pub struct FakeFiles(pub Shared);
pub struct FakeArchive(pub Shared);
pub struct FakeHasher(pub Shared);

impl ModRepository for FakeRepo {
    fn list(&self) -> Result<Vec<ModEntry>, LibraryError> {
        Ok(self.0.borrow().index.clone())
    }

    fn add_all(&self, entries: &[ModEntry]) -> Result<(), LibraryError> {
        if self.0.borrow().fail_index_add {
            return Err(io_error(Path::new("library.json"), "index write failed"));
        }
        self.0.borrow_mut().index.extend(entries.iter().cloned());
        Ok(())
    }

    fn remove(&self, id: &str) -> Result<bool, LibraryError> {
        let mut state = self.0.borrow_mut();
        let before = state.index.len();
        state.index.retain(|e| e.id != id);
        Ok(state.index.len() != before)
    }
}

impl LibraryFiles for FakeFiles {
    fn list_dir(&self, dir: &Path) -> Result<Vec<String>, LibraryError> {
        Ok(self
            .0
            .borrow()
            .files
            .keys()
            .filter(|p| p.parent() == Some(dir))
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect())
    }

    fn create_staging(&self) -> Result<PathBuf, LibraryError> {
        let mut state = self.0.borrow_mut();
        state.staging_counter += 1;
        Ok(library_dir().join(format!(".staging-{}", state.staging_counter)))
    }

    fn copy_file(&self, source: &Path, dest_dir: &Path) -> Result<(), LibraryError> {
        let mut state = self.0.borrow_mut();
        if state.fail_copy {
            return Err(io_error(source, "copy failed"));
        }
        let content = state
            .files
            .get(source)
            .cloned()
            .ok_or_else(|| io_error(source, "missing source"))?;
        state
            .files
            .insert(dest_dir.join(source.file_name().unwrap()), content);
        Ok(())
    }

    fn file_size(&self, file: &Path) -> Result<u64, LibraryError> {
        self.0
            .borrow()
            .files
            .get(file)
            .map(|c| c.len() as u64)
            .ok_or_else(|| io_error(file, "missing file"))
    }

    fn publish(&self, staging: &Path, files: &[String], id: &str) -> Result<(), LibraryError> {
        if self.0.borrow().fail_publish_id.as_deref() == Some(id) {
            return Err(io_error(staging, "publish failed"));
        }
        let mut state = self.0.borrow_mut();
        for name in files {
            let content = state
                .files
                .remove(&staging.join(name))
                .ok_or_else(|| io_error(staging, "missing staged file"))?;
            state
                .files
                .insert(library_dir().join(id).join(name), content);
        }
        Ok(())
    }

    fn discard(&self, staging: &Path) -> Result<(), LibraryError> {
        let mut state = self.0.borrow_mut();
        state.files.retain(|p, _| !p.starts_with(staging));
        state.discarded.push(staging.to_path_buf());
        Ok(())
    }

    fn remove_mod_dir(&self, id: &str) -> Result<(), LibraryError> {
        if self.0.borrow().fail_remove_dir {
            return Err(io_error(Path::new(id), "sharing violation"));
        }
        let dir = library_dir().join(id);
        self.0
            .borrow_mut()
            .files
            .retain(|p, _| !p.starts_with(&dir));
        Ok(())
    }
}

impl ArchiveReader for FakeArchive {
    fn extract_flat(&self, archive: &Path, dest_dir: &Path) -> Result<Vec<String>, LibraryError> {
        let mut state = self.0.borrow_mut();
        let entries = state
            .archives
            .get(archive)
            .cloned()
            .ok_or_else(|| io_error(archive, "missing archive"))?;
        let mut names = Vec::new();
        for (name, content) in entries {
            state.files.insert(dest_dir.join(&name), content);
            names.push(name);
        }
        Ok(names)
    }
}

impl ContentHasher for FakeHasher {
    fn sha256_hex(&self, file: &Path) -> Result<String, LibraryError> {
        self.0
            .borrow()
            .files
            .get(file)
            .map(|c| fake_hash(c))
            .ok_or_else(|| io_error(file, "missing file"))
    }
}

pub type FakeService = ModService<FakeRepo, FakeFiles, FakeArchive, FakeHasher>;

pub fn service(state: &Shared) -> FakeService {
    ModService::new(
        FakeRepo(state.clone()),
        FakeFiles(state.clone()),
        FakeArchive(state.clone()),
        FakeHasher(state.clone()),
    )
}

/// Paths of the files stored under `library/<id>/`.
pub fn library_files_of(state: &Shared, id: &str) -> Vec<String> {
    let dir = library_dir().join(id);
    state
        .borrow()
        .files
        .keys()
        .filter(|p| p.parent() == Some(dir.as_path()))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

/// Any leftover file inside a staging directory.
pub fn staging_leftovers(state: &Shared) -> Vec<PathBuf> {
    state
        .borrow()
        .files
        .keys()
        .filter(|p| {
            p.components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with(".staging-"))
        })
        .cloned()
        .collect()
}
