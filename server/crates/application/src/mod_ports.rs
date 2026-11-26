//! Ports of mod hosting: file storage and the mod repository.

use chrono::{DateTime, Utc};
use community_domain::mods::{
    ModMetadata, ModSlug, ModUpload, ModVersion, PublishedMod, Screenshot, StorageKey,
    VersionStatus,
};
use community_domain::UserId;

use crate::ports::PortResult;

/// File storage with three areas: `incoming` (uploads in progress, by upload
/// id), `pending` (completed, waiting for review) and `public` (approved,
/// downloadable). Keys are built by the server only. Files are never executed
/// or extracted.
#[async_trait::async_trait]
pub trait ModStorage: Send + Sync {
    /// Writes `bytes` at `offset` of the incoming file, dropping anything past it
    /// (so a retry after a half-written chunk is safe).
    async fn append_incoming(&self, upload_id: &str, offset: u64, bytes: &[u8]) -> PortResult<()>;
    /// Bytes stored for the upload (0 when none).
    async fn incoming_len(&self, upload_id: &str) -> PortResult<u64>;
    /// Lowercase hex SHA-256 of the incoming file, streamed.
    async fn sha256_incoming(&self, upload_id: &str) -> PortResult<String>;
    /// Moves the incoming file into the pending area under `key`.
    async fn commit_incoming(&self, upload_id: &str, key: &StorageKey) -> PortResult<()>;
    /// Stores a small file (a screenshot) in the pending area.
    async fn put_pending(&self, key: &StorageKey, bytes: &[u8]) -> PortResult<()>;
    /// Moves a pending file to the public area (idempotent).
    async fn publish(&self, key: &StorageKey) -> PortResult<()>;
    /// Removes the file from the pending and public areas (missing is fine).
    async fn delete(&self, key: &StorageKey) -> PortResult<()>;
    /// Removes the incoming file (missing is fine).
    async fn delete_incoming(&self, upload_id: &str) -> PortResult<()>;
}

/// The version row created when an upload completes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewVersion {
    pub id: String,
    pub owner: UserId,
    pub meta: ModMetadata,
    pub sha256: String,
    pub storage_key: StorageKey,
    pub uploaded_at: DateTime<Utc>,
}

#[async_trait::async_trait]
pub trait ModRepository: Send + Sync {
    async fn mod_owner(&self, mod_id: &ModSlug) -> PortResult<Option<UserId>>;
    async fn find_version(&self, mod_id: &ModSlug, version: &str)
        -> PortResult<Option<ModVersion>>;

    /// Creates the session and records the start (for the per-day limit).
    async fn create_upload(&self, upload: &ModUpload) -> PortResult<()>;
    async fn get_upload(&self, id: &str) -> PortResult<Option<ModUpload>>;
    async fn set_received(&self, id: &str, received: u64) -> PortResult<()>;
    /// Removes the session and its screenshot rows.
    async fn delete_upload(&self, id: &str) -> PortResult<()>;
    async fn expired_uploads(&self, now: DateTime<Utc>) -> PortResult<Vec<ModUpload>>;
    /// Start times of the user's uploads since `since` (aborted ones included).
    async fn upload_starts_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> PortResult<Vec<DateTime<Utc>>>;
    /// Bytes held by the user: non-rejected versions plus the declared size of open uploads.
    async fn stored_bytes(&self, user: &UserId) -> PortResult<u64>;

    async fn add_upload_screenshot(
        &self,
        upload_id: &str,
        screenshot: &Screenshot,
    ) -> PortResult<()>;
    async fn upload_screenshots(&self, upload_id: &str) -> PortResult<Vec<Screenshot>>;

    /// Atomically creates the mod (when new) and the `pending` version, moves the
    /// upload's screenshots to it and removes the session. A rejected version with
    /// the same number is replaced. Fails with `ModIdTaken` / `VersionExists`.
    async fn complete_upload(
        &self,
        upload_id: &str,
        version: &NewVersion,
    ) -> PortResult<ModVersion>;

    /// Every version of the user's mods, oldest first within a mod.
    async fn versions_of_user(&self, user: &UserId) -> PortResult<Vec<ModVersion>>;
    async fn pending_versions(&self) -> PortResult<Vec<ModVersion>>;
    /// Sets the review result. `Rejected` also drops the screenshot rows.
    async fn review(
        &self,
        version_id: &str,
        status: VersionStatus,
        reason: Option<&str>,
        reviewer: &UserId,
        now: DateTime<Utc>,
    ) -> PortResult<()>;
    /// The latest approved version of every mod, with the author's display name.
    async fn latest_approved(&self) -> PortResult<Vec<PublishedMod>>;
}
