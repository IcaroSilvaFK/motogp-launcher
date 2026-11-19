//! `DeployFiles` adapter: hard link with an exclusive-copy fallback.
use motogp_application::{DeployError, DeployFiles, DirCheck};
use motogp_domain::is_safe_file_name;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Read};
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

/// Copies `source` into a freshly created `dest` (`create_new`).
fn copy_exclusive(source: &Path, dest: &Path, name: &str) -> Result<(), DeployError> {
    let mut input = fs::File::open(source).map_err(|e| io_error(source, &e))?;
    copy_into_new(&mut input, dest, name, |path| fs::remove_file(path))
}

/// Creates `dest` exclusively and fills it from `input`. A failure after the
/// destination was created removes it with `remove`; if that removal fails the
/// copy error is still reported and the file stays (the journal entry keeps it
/// recoverable, and ownership is re-verified before it is ever deleted).
fn copy_into_new<R: Read>(
    input: &mut R,
    dest: &Path,
    name: &str,
    remove: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), DeployError> {
    let mut output = match OpenOptions::new().write(true).create_new(true).open(dest) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(DeployError::AlreadyExists { name: name.into() })
        }
        Err(e) => return Err(io_error(dest, &e)),
    };
    let copied = io::copy(input, &mut output).and_then(|_| output.sync_all());
    if let Err(e) = copied {
        drop(output);
        let _ = remove(dest);
        return Err(io_error(dest, &e));
    }
    Ok(())
}

/// Canonical form used to compare directories; `None` when it does not exist.
fn canonical(path: &Path) -> Result<Option<PathBuf>, DeployError> {
    match fs::canonicalize(path) {
        Ok(resolved) => Ok(Some(resolved)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, &e)),
    }
}

/// Byte-for-byte equality of two files of the same length.
fn same_content(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut fa, mut fb) = (fs::File::open(a)?, fs::File::open(b)?);
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    let (mut ba, mut bb) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    loop {
        let n = fa.read(&mut ba)?;
        if n == 0 {
            return Ok(fb.read(&mut bb)? == 0);
        }
        fb.read_exact(&mut bb[..n])?;
        if ba[..n] != bb[..n] {
            return Ok(false);
        }
    }
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

    fn check_dir(&self, manifest_dir: &Path, authorized: &Path) -> Result<DirCheck, DeployError> {
        let Some(found) = canonical(manifest_dir)? else {
            return Ok(DirCheck::Missing);
        };
        Ok(match canonical(authorized)? {
            Some(expected) if expected == found => DirCheck::Authorized,
            _ => DirCheck::Foreign,
        })
    }

    fn owns(&self, dir: &Path, name: &str, source: &Path) -> Result<bool, DeployError> {
        let path = destination(dir, name)?;
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_file() => {}
            Ok(_) => return Ok(false),
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(io_error(&path, &e)),
        }
        // Without the library file there is no proof: leave the destination.
        if !source.is_file() {
            return Ok(false);
        }
        // A hard link is the very same file as its library source.
        if same_file::is_same_file(&path, source).unwrap_or(false) {
            return Ok(true);
        }
        // The copy fallback is a faithful byte copy of the library file.
        same_content(&path, source).map_err(|e| io_error(&path, &e))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// Yields some bytes, then fails like a disk or network read error.
    struct FlakyReader {
        sent: bool,
    }

    impl Read for FlakyReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.sent {
                return Err(io::Error::new(io::ErrorKind::Other, "read failed"));
            }
            self.sent = true;
            buf[..4].copy_from_slice(b"part");
            Ok(4)
        }
    }

    fn failing_remove(_: &Path) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked"))
    }

    #[test]
    fn a_copy_that_fails_midway_removes_the_destination_it_created() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.pak");

        let error = copy_into_new(&mut FlakyReader { sent: false }, &dest, "x.pak", |p| {
            fs::remove_file(p)
        })
        .unwrap_err();

        assert!(matches!(error, DeployError::Io { .. }), "{error}");
        assert!(error.to_string().contains("read failed"), "{error}");
        assert!(!dest.exists(), "the partial destination must be removed");
    }

    #[test]
    fn a_failed_removal_leaves_the_partial_file_and_still_reports_the_copy_error() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.pak");

        let error = copy_into_new(
            &mut FlakyReader { sent: false },
            &dest,
            "x.pak",
            failing_remove,
        )
        .unwrap_err();

        // The error stays the copy failure; the file is left for the journal
        // entry (which the service keeps) to recover.
        assert!(error.to_string().contains("read failed"), "{error}");
        assert!(dest.exists());
    }

    #[test]
    fn a_copy_never_overwrites_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("x.pak");
        fs::write(&dest, b"user").unwrap();

        let error =
            copy_into_new(&mut &b"new"[..], &dest, "x.pak", |p| fs::remove_file(p)).unwrap_err();

        assert!(matches!(error, DeployError::AlreadyExists { .. }));
        assert_eq!(fs::read(&dest).unwrap(), b"user");
    }
}
