//! Local-disk adapter of the mod file storage.
//!
//! Layout under the root (`MOD_STORAGE_DIR`): `incoming/<upload id>` (uploads in
//! progress), `pending/<mod>/<version>/<file>` (waiting for review) and
//! `public/<mod>/<version>/<file>` (approved; the only area ever served).
//! Names come from validated parts; every path is re-checked here so nothing
//! can escape the root.

use std::fmt::Write as _;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use community_application::mod_ports::ModStorage;
use community_application::ports::PortResult;
use community_application::AppError;
use community_domain::mods::StorageKey;
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const HASH_BUFFER: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct LocalModStorage {
    root: PathBuf,
}

fn io_error(error: std::io::Error) -> AppError {
    AppError::Internal(format!("storage error: {error}"))
}

fn not_found_ok(result: std::io::Result<()>) -> PortResult<()> {
    match result {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(io_error(error)),
        _ => Ok(()),
    }
}

/// An upload id is a server-made base64url token.
fn valid_upload_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+' | b' ' | b'(' | b')')
        })
}

impl LocalModStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory that may be served over HTTP (approved files only).
    pub fn public_dir(&self) -> PathBuf {
        self.root.join("public")
    }

    /// Creates the three areas.
    pub async fn ensure_dirs(&self) -> std::io::Result<()> {
        for area in ["incoming", "pending", "public"] {
            fs::create_dir_all(self.root.join(area)).await?;
        }
        Ok(())
    }

    fn incoming_path(&self, upload_id: &str) -> PortResult<PathBuf> {
        if !valid_upload_id(upload_id) {
            return Err(AppError::Internal("invalid upload id".into()));
        }
        Ok(self.root.join("incoming").join(upload_id))
    }

    fn area_path(&self, area: &str, key: &StorageKey) -> PortResult<PathBuf> {
        let mut path = self.root.join(area);
        let mut segments = 0;
        for segment in key.as_str().split('/') {
            if !valid_segment(segment) {
                return Err(AppError::Internal("invalid storage key".into()));
            }
            path.push(segment);
            segments += 1;
        }
        if segments != 3 {
            return Err(AppError::Internal("invalid storage key".into()));
        }
        Ok(path)
    }

    async fn ensure_parent(path: &Path) -> PortResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(io_error)?;
        }
        Ok(())
    }

    /// Removes empty `<mod>/<version>` folders up to the area root (best effort).
    async fn prune_empty_dirs(&self, area: &str, file: &Path) {
        let stop = self.root.join(area);
        let mut current = file.parent().map(Path::to_path_buf);
        while let Some(dir) = current {
            if dir == stop || !dir.starts_with(&stop) || fs::remove_dir(&dir).await.is_err() {
                break;
            }
            current = dir.parent().map(Path::to_path_buf);
        }
    }
}

#[async_trait::async_trait]
impl ModStorage for LocalModStorage {
    async fn append_incoming(&self, upload_id: &str, offset: u64, bytes: &[u8]) -> PortResult<()> {
        let path = self.incoming_path(upload_id)?;
        Self::ensure_parent(&path).await?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .await
            .map_err(io_error)?;
        let length = file.metadata().await.map_err(io_error)?.len();
        if length < offset {
            return Err(AppError::OffsetMismatch { received: length });
        }
        // Drop a half-written chunk from an earlier attempt, then write at the offset.
        file.set_len(offset).await.map_err(io_error)?;
        file.seek(SeekFrom::Start(offset)).await.map_err(io_error)?;
        file.write_all(bytes).await.map_err(io_error)?;
        file.flush().await.map_err(io_error)
    }

    async fn incoming_len(&self, upload_id: &str) -> PortResult<u64> {
        let path = self.incoming_path(upload_id)?;
        match fs::metadata(&path).await {
            Ok(meta) => Ok(meta.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(io_error(error)),
        }
    }

    async fn sha256_incoming(&self, upload_id: &str) -> PortResult<String> {
        let path = self.incoming_path(upload_id)?;
        let mut file = fs::File::open(&path).await.map_err(io_error)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; HASH_BUFFER];
        loop {
            let read = file.read(&mut buffer).await.map_err(io_error)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let mut hex = String::with_capacity(64);
        for byte in hasher.finalize() {
            let _ = write!(hex, "{byte:02x}");
        }
        Ok(hex)
    }

    async fn commit_incoming(&self, upload_id: &str, key: &StorageKey) -> PortResult<()> {
        let from = self.incoming_path(upload_id)?;
        let to = self.area_path("pending", key)?;
        Self::ensure_parent(&to).await?;
        fs::rename(&from, &to).await.map_err(io_error)
    }

    async fn put_pending(&self, key: &StorageKey, bytes: &[u8]) -> PortResult<()> {
        let path = self.area_path("pending", key)?;
        Self::ensure_parent(&path).await?;
        fs::write(&path, bytes).await.map_err(io_error)
    }

    async fn publish(&self, key: &StorageKey) -> PortResult<()> {
        let from = self.area_path("pending", key)?;
        let to = self.area_path("public", key)?;
        if fs::metadata(&from).await.is_err() {
            // Already published by an earlier attempt.
            return fs::metadata(&to)
                .await
                .map(|_| ())
                .map_err(|_| AppError::Internal("the file to publish is missing".into()));
        }
        Self::ensure_parent(&to).await?;
        fs::rename(&from, &to).await.map_err(io_error)?;
        self.prune_empty_dirs("pending", &from).await;
        Ok(())
    }

    async fn delete(&self, key: &StorageKey) -> PortResult<()> {
        for area in ["pending", "public"] {
            let path = self.area_path(area, key)?;
            not_found_ok(fs::remove_file(&path).await)?;
            self.prune_empty_dirs(area, &path).await;
        }
        Ok(())
    }

    async fn delete_incoming(&self, upload_id: &str) -> PortResult<()> {
        let path = self.incoming_path(upload_id)?;
        not_found_ok(fs::remove_file(&path).await)
    }

    async fn locate(&self, key: &StorageKey) -> PortResult<Option<PathBuf>> {
        for area in ["pending", "public"] {
            let path = self.area_path(area, key)?;
            match fs::metadata(&path).await {
                Ok(meta) if meta.is_file() => return Ok(Some(path)),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(io_error(error)),
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use community_domain::mods::{ImageKind, ModSlug};

    use super::*;

    async fn storage() -> (tempfile::TempDir, LocalModStorage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = LocalModStorage::new(dir.path());
        storage.ensure_dirs().await.unwrap();
        (dir, storage)
    }

    fn key(file: &str) -> StorageKey {
        StorageKey::file(&ModSlug::parse("brazil-livery").unwrap(), "1.0", file)
    }

    #[tokio::test]
    async fn locate_finds_pending_then_public_files_only() {
        let (_dir, s) = storage().await;
        let k = key("a.zip");
        assert_eq!(s.locate(&k).await.unwrap(), None);
        s.put_pending(&k, b"x").await.unwrap();
        let pending = s.locate(&k).await.unwrap().unwrap();
        assert!(pending.starts_with(s.root.join("pending")));
        s.publish(&k).await.unwrap();
        let public = s.locate(&k).await.unwrap().unwrap();
        assert!(public.starts_with(s.public_dir()));
    }

    #[tokio::test]
    async fn chunks_append_in_order_and_hash_like_sha256() {
        let (_dir, s) = storage().await;
        assert_eq!(s.incoming_len("u1").await.unwrap(), 0);
        s.append_incoming("u1", 0, b"hello ").await.unwrap();
        s.append_incoming("u1", 6, b"world").await.unwrap();
        assert_eq!(s.incoming_len("u1").await.unwrap(), 11);
        assert_eq!(
            s.sha256_incoming("u1").await.unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[tokio::test]
    async fn a_retried_chunk_replaces_the_half_written_one() {
        let (_dir, s) = storage().await;
        s.append_incoming("u1", 0, b"abcdef").await.unwrap();
        // The server lost track after 3 bytes: the retry rewrites from offset 3.
        s.append_incoming("u1", 3, b"XYZ").await.unwrap();
        assert_eq!(s.incoming_len("u1").await.unwrap(), 6);
        s.append_incoming("u1", 6, b"!").await.unwrap();
        assert_eq!(
            fs::read(s.root.join("incoming/u1")).await.unwrap(),
            b"abcXYZ!"
        );
    }

    #[tokio::test]
    async fn an_offset_past_the_end_is_refused() {
        let (_dir, s) = storage().await;
        s.append_incoming("u1", 0, b"abc").await.unwrap();
        assert_eq!(
            s.append_incoming("u1", 9, b"x").await.unwrap_err(),
            AppError::OffsetMismatch { received: 3 }
        );
    }

    #[tokio::test]
    async fn commit_then_publish_moves_the_file_between_areas() {
        let (dir, s) = storage().await;
        s.append_incoming("u1", 0, b"data").await.unwrap();
        s.commit_incoming("u1", &key("a.zip")).await.unwrap();
        let pending = dir.path().join("pending/brazil-livery/1.0/a.zip");
        let public = dir.path().join("public/brazil-livery/1.0/a.zip");
        assert!(pending.exists() && !public.exists());
        assert_eq!(s.incoming_len("u1").await.unwrap(), 0);

        s.publish(&key("a.zip")).await.unwrap();
        assert!(!pending.exists() && public.exists());
        assert!(!dir.path().join("pending/brazil-livery").exists());
        // Idempotent.
        s.publish(&key("a.zip")).await.unwrap();
        assert!(s.publish(&key("missing.zip")).await.is_err());
    }

    #[tokio::test]
    async fn screenshots_and_delete_clean_both_areas() {
        let (dir, s) = storage().await;
        let shot = StorageKey::screenshot(
            &ModSlug::parse("brazil-livery").unwrap(),
            "1.0",
            0,
            ImageKind::Png,
        );
        s.put_pending(&shot, b"png").await.unwrap();
        s.publish(&shot).await.unwrap();
        s.delete(&shot).await.unwrap();
        assert!(!dir.path().join("public/brazil-livery").exists());
        // Deleting what is gone is fine.
        s.delete(&shot).await.unwrap();
        s.delete_incoming("nothing").await.unwrap();
        s.append_incoming("u2", 0, b"x").await.unwrap();
        s.delete_incoming("u2").await.unwrap();
        assert_eq!(s.incoming_len("u2").await.unwrap(), 0);
    }

    #[tokio::test]
    async fn path_traversal_and_bad_ids_are_rejected() {
        let (dir, s) = storage().await;
        for bad in [
            "../evil/1.0/a.zip",
            "brazil-livery/../../a.zip",
            "brazil-livery/1.0/../a.zip",
            "brazil-livery/1.0/a/b.zip",
            "brazil-livery/1.0",
            "/etc/passwd",
            "brazil-livery\\1.0\\a.zip",
            "brazil-livery/1.0/",
        ] {
            let key = StorageKey::from_stored(bad.into());
            assert!(s.put_pending(&key, b"x").await.is_err(), "{bad}");
            assert!(s.delete(&key).await.is_err(), "{bad}");
        }
        for bad in ["../x", "a/b", "", "a b", "a\\b"] {
            assert!(s.append_incoming(bad, 0, b"x").await.is_err(), "{bad:?}");
            assert!(s.incoming_len(bad).await.is_err(), "{bad:?}");
        }
        assert!(!dir.path().join("evil").exists());
    }

    #[test]
    fn the_public_dir_is_the_only_served_area() {
        let s = LocalModStorage::new("/data/mods");
        assert_eq!(s.public_dir(), PathBuf::from("/data/mods/public"));
    }
}
