//! `DeployFiles` adapter: hard link with an exclusive-copy fallback.
use motogp_application::{DeployError, DeployFiles};
use motogp_domain::is_safe_file_name;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// Creates a hard link `dest` pointing at `source`.
pub type LinkFn = fn(&Path, &Path) -> io::Result<()>;

/// File-system adapter of the game `~mods` folder.
///
/// Files are hard-linked (no extra disk usage); when linking fails for any
/// reason other than an existing destination (for example across volumes) the
/// file is copied. Both paths create the destination exclusively and never
/// overwrite.
#[derive(Debug, Clone, Copy)]
pub struct FsDeployFiles {
    link: LinkFn,
}

impl Default for FsDeployFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl FsDeployFiles {
    pub fn new() -> Self {
        Self::with_link(|source, dest| fs::hard_link(source, dest))
    }

    /// Injects the link operation (tests use a failing one to exercise the
    /// copy fallback without two volumes).
    pub fn with_link(link: LinkFn) -> Self {
        Self { link }
    }
}

fn io_error(path: &Path, error: &io::Error) -> DeployError {
    DeployError::Io {
        location: path.display().to_string(),
        detail: error.to_string(),
    }
}

/// Defense in depth: the adapter never joins an unsafe name onto a directory.
fn destination(dir: &Path, name: &str) -> Result<PathBuf, DeployError> {
    if is_safe_file_name(name) {
        Ok(dir.join(name))
    } else {
        Err(DeployError::InvalidPath { path: name.into() })
    }
}

/// Copies `source` into a freshly created `dest` (`create_new`); a partial
/// destination created by a failed copy is removed.
fn copy_exclusive(source: &Path, dest: &Path, name: &str) -> Result<(), DeployError> {
    let mut input = fs::File::open(source).map_err(|e| io_error(source, &e))?;
    let mut output = match OpenOptions::new().write(true).create_new(true).open(dest) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(DeployError::AlreadyExists { name: name.into() })
        }
        Err(e) => return Err(io_error(dest, &e)),
    };
    let copied = io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
    if let Err(e) = copied {
        drop(output);
        let _ = fs::remove_file(dest);
        return Err(io_error(dest, &e));
    }
    Ok(())
}

impl DeployFiles for FsDeployFiles {
    fn exists(&self, dir: &Path, name: &str) -> Result<bool, DeployError> {
        let path = destination(dir, name)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error(&path, &e)),
        }
    }

    fn create_dir_all(&self, dir: &Path) -> Result<(), DeployError> {
        fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))
    }

    fn materialize(&self, source: &Path, dir: &Path, name: &str) -> Result<(), DeployError> {
        let dest = destination(dir, name)?;
        match (self.link)(source, &dest) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                Err(DeployError::AlreadyExists { name: name.into() })
            }
            Err(_) => copy_exclusive(source, &dest, name),
        }
    }

    fn remove(&self, dir: &Path, name: &str) -> Result<bool, DeployError> {
        let path = destination(dir, name)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error(&path, &e)),
        }
    }
}
