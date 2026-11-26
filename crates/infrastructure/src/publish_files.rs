//! `PublishFiles` adapter: reads the local mod file by ranges and hashes it
//! with SHA-256 in fixed-size chunks (mods can be gigabytes).
use motogp_application::PublishFiles;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const HASH_CHUNK: usize = 256 * 1024;

#[derive(Debug, Default, Clone, Copy)]
pub struct FsPublishFiles;

impl PublishFiles for FsPublishFiles {
    fn size(&self, path: &Path) -> Result<u64, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("not a file".to_string());
        }
        Ok(meta.len())
    }

    fn sha256_hex(&self, path: &Path, progress: &mut dyn FnMut(u64)) -> Result<String, String> {
        let mut input = File::open(path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; HASH_CHUNK];
        let mut done = 0u64;
        loop {
            let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            done += read as u64;
            progress(done);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, String> {
        let mut input = File::open(path).map_err(|e| e.to_string())?;
        input
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        input
            .take(len)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn file(content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod.zip");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn size_and_ranges_follow_the_file() {
        let (_dir, path) = file(b"0123456789");
        assert_eq!(FsPublishFiles.size(&path).unwrap(), 10);
        assert_eq!(FsPublishFiles.read_range(&path, 2, 4).unwrap(), b"2345");
        // A range running past the end is cut at the end.
        assert_eq!(FsPublishFiles.read_range(&path, 8, 100).unwrap(), b"89");
        assert!(FsPublishFiles.read_range(&path, 50, 4).unwrap().is_empty());
    }

    #[test]
    fn the_hash_matches_a_known_digest_and_reports_progress() {
        let (_dir, path) = file(b"abc");
        let mut seen = Vec::new();
        let hash = FsPublishFiles
            .sha256_hex(&path, &mut |done| seen.push(done))
            .unwrap();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(seen, vec![3]);
    }

    #[test]
    fn a_large_file_is_hashed_in_several_steps() {
        let (_dir, path) = file(&vec![7u8; HASH_CHUNK * 2 + 5]);
        let mut seen = Vec::new();
        FsPublishFiles
            .sha256_hex(&path, &mut |done| seen.push(done))
            .unwrap();
        assert!(seen.len() >= 3);
        assert_eq!(*seen.last().unwrap(), (HASH_CHUNK * 2 + 5) as u64);
    }

    #[test]
    fn missing_files_and_folders_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.zip");
        assert!(FsPublishFiles.size(&missing).is_err());
        assert!(FsPublishFiles.size(dir.path()).is_err());
        assert!(FsPublishFiles.sha256_hex(&missing, &mut |_| {}).is_err());
        assert!(FsPublishFiles.read_range(&missing, 0, 1).is_err());
    }
}
