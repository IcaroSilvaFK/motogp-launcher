//! Mod hosting use cases: resumable uploads, review and the public catalog.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use community_domain::mods::{
    ImageKind, ModMetadata, ModRuleError, ModSlug, ModUpload, ModVersion, PublishedMod,
    RawMetadata, Screenshot, StorageKey, VersionStatus, CHUNK_SIZE, MAX_REASON_CHARS,
    MAX_SCREENSHOTS, MAX_SCREENSHOT_BYTES, UPLOAD_LIFETIME_HOURS,
};
use community_domain::{DiscordId, User};

use crate::mod_ports::{ModRepository, ModStorage, NewVersion};
use crate::ports::{Clock, PortResult, TokenGenerator};
use crate::AppError;

/// Server-side limits (all configurable).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModLimits {
    pub max_upload_bytes: u64,
    pub user_quota_bytes: u64,
    pub uploads_per_day: u32,
}

/// What `POST /mods/uploads` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadSession {
    pub upload_id: String,
    pub chunk_size: u64,
    pub received: u64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadState {
    Uploading,
    /// Every byte arrived; the upload can be completed.
    Ready,
}

impl UploadState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Uploading => "uploading",
            Self::Ready => "ready",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadStatus {
    pub received: u64,
    pub size: u64,
    pub state: UploadState,
}

/// One of the user's mods with every version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MyMod {
    pub mod_id: ModSlug,
    pub versions: Vec<ModVersion>,
}

pub struct ModHostingService {
    pub clock: Arc<dyn Clock>,
    pub tokens: Arc<dyn TokenGenerator>,
    pub repo: Arc<dyn ModRepository>,
    pub storage: Arc<dyn ModStorage>,
    /// Discord ids allowed to review.
    pub admin_discord_ids: Vec<DiscordId>,
    pub limits: ModLimits,
}

fn rule(error: ModRuleError) -> AppError {
    match error {
        ModRuleError::Invalid(message) => AppError::BadRequest(message),
        ModRuleError::UnsupportedFile(message) => AppError::UnsupportedFile(message),
    }
}

impl ModHostingService {
    pub fn is_admin(&self, user: &User) -> bool {
        self.admin_discord_ids.contains(&user.discord_id)
    }

    fn require_admin(&self, user: &User) -> PortResult<()> {
        self.is_admin(user).then_some(()).ok_or(AppError::Forbidden)
    }

    /// Starts a resumable upload. Everything that can be refused is refused here,
    /// before a single byte is stored.
    pub async fn create_upload(&self, user: &User, raw: RawMetadata) -> PortResult<UploadSession> {
        let now = self.clock.now();
        // Opportunistic cleanup; a failure must not block the upload.
        let _ = self.purge_expired_uploads().await;

        let meta = ModMetadata::validate(raw).map_err(rule)?;
        if meta.size > self.limits.max_upload_bytes {
            return Err(AppError::FileTooLarge);
        }
        if let Some(owner) = self.repo.mod_owner(&meta.mod_id).await? {
            if owner != user.id {
                return Err(AppError::ModIdTaken);
            }
        }
        if let Some(existing) = self.repo.find_version(&meta.mod_id, &meta.version).await? {
            if existing.status != VersionStatus::Rejected {
                return Err(AppError::VersionExists);
            }
        }

        let window = Duration::hours(UPLOAD_LIFETIME_HOURS);
        let starts = self
            .repo
            .upload_starts_since(&user.id, now - window)
            .await?;
        if starts.len() >= self.limits.uploads_per_day as usize {
            let oldest = starts.iter().min().copied().unwrap_or(now);
            let wait = (oldest + window - now).num_seconds().max(1);
            return Err(AppError::RateLimited {
                retry_after_secs: wait as u64,
            });
        }
        let stored = self.repo.stored_bytes(&user.id).await?;
        if stored.saturating_add(meta.size) > self.limits.user_quota_bytes {
            return Err(AppError::QuotaExceeded);
        }

        let upload = ModUpload {
            id: self.tokens.generate(16),
            user_id: user.id.clone(),
            meta,
            received: 0,
            created_at: now,
            expires_at: now + window,
        };
        self.repo.create_upload(&upload).await?;
        Ok(UploadSession {
            upload_id: upload.id,
            chunk_size: CHUNK_SIZE,
            received: 0,
            expires_at: upload.expires_at,
        })
    }

    async fn owned_upload(&self, user: &User, upload_id: &str) -> PortResult<ModUpload> {
        let upload = self
            .repo
            .get_upload(upload_id)
            .await?
            .filter(|upload| upload.expires_at > self.clock.now())
            .ok_or(AppError::NotFound)?;
        if upload.user_id != user.id {
            return Err(AppError::Forbidden);
        }
        Ok(upload)
    }

    /// Stores the next chunk. `offset` must equal the bytes already received.
    pub async fn write_chunk(
        &self,
        user: &User,
        upload_id: &str,
        offset: u64,
        bytes: &[u8],
    ) -> PortResult<u64> {
        let upload = self.owned_upload(user, upload_id).await?;
        if offset != upload.received {
            return Err(AppError::OffsetMismatch {
                received: upload.received,
            });
        }
        let length = bytes.len() as u64;
        if length == 0 {
            return Err(AppError::BadRequest("The chunk is empty.".into()));
        }
        if length > CHUNK_SIZE {
            return Err(AppError::FileTooLarge);
        }
        let received = upload.received + length;
        if received > upload.meta.size {
            return Err(AppError::BadRequest(
                "The chunk goes past the declared file size.".into(),
            ));
        }
        if received > self.limits.max_upload_bytes {
            return Err(AppError::FileTooLarge);
        }
        self.storage
            .append_incoming(upload_id, offset, bytes)
            .await?;
        self.repo.set_received(upload_id, received).await?;
        Ok(received)
    }

    /// Resume information.
    pub async fn upload_status(&self, user: &User, upload_id: &str) -> PortResult<UploadStatus> {
        let upload = self.owned_upload(user, upload_id).await?;
        let state = if upload.received == upload.meta.size {
            UploadState::Ready
        } else {
            UploadState::Uploading
        };
        Ok(UploadStatus {
            received: upload.received,
            size: upload.meta.size,
            state,
        })
    }

    /// Adds a screenshot (PNG, JPEG or WebP by magic bytes) to the upload; returns its index.
    pub async fn add_screenshot(
        &self,
        user: &User,
        upload_id: &str,
        bytes: &[u8],
    ) -> PortResult<usize> {
        let upload = self.owned_upload(user, upload_id).await?;
        if bytes.len() as u64 > MAX_SCREENSHOT_BYTES {
            return Err(AppError::FileTooLarge);
        }
        let kind = ImageKind::sniff(bytes).ok_or_else(|| {
            AppError::UnsupportedFile("Screenshots must be PNG, JPEG or WebP images.".into())
        })?;
        let existing = self.repo.upload_screenshots(upload_id).await?;
        if existing.len() >= MAX_SCREENSHOTS {
            return Err(AppError::BadRequest(format!(
                "At most {MAX_SCREENSHOTS} screenshots are allowed."
            )));
        }
        let index = existing.len();
        let screenshot = Screenshot {
            index,
            storage_key: StorageKey::screenshot(
                &upload.meta.mod_id,
                &upload.meta.version,
                index,
                kind,
            ),
            content_type: kind.content_type().to_owned(),
        };
        self.storage
            .put_pending(&screenshot.storage_key, bytes)
            .await?;
        self.repo
            .add_upload_screenshot(upload_id, &screenshot)
            .await?;
        Ok(index)
    }

    /// Verifies the bytes, moves the file to the pending area and creates the
    /// `pending` version.
    pub async fn complete(&self, user: &User, upload_id: &str) -> PortResult<ModVersion> {
        let upload = self.owned_upload(user, upload_id).await?;
        if upload.received != upload.meta.size {
            return Err(AppError::UploadIncomplete);
        }
        let stored = self.storage.incoming_len(upload_id).await?;
        if stored != upload.meta.size {
            // The bytes on disk do not match the books: let the client resume.
            self.repo
                .set_received(upload_id, stored.min(upload.received))
                .await?;
            return Err(AppError::UploadIncomplete);
        }
        let sha256 = self.storage.sha256_incoming(upload_id).await?;
        if let Some(expected) = &upload.meta.client_sha256 {
            if *expected != sha256 {
                self.discard(&upload).await?;
                return Err(AppError::ChecksumMismatch);
            }
        }
        // Re-check what may have changed since the upload started.
        if let Some(owner) = self.repo.mod_owner(&upload.meta.mod_id).await? {
            if owner != user.id {
                return Err(AppError::ModIdTaken);
            }
        }
        if let Some(existing) = self
            .repo
            .find_version(&upload.meta.mod_id, &upload.meta.version)
            .await?
        {
            if existing.status != VersionStatus::Rejected {
                return Err(AppError::VersionExists);
            }
        }

        let key = StorageKey::file(
            &upload.meta.mod_id,
            &upload.meta.version,
            &upload.meta.file_name,
        );
        self.storage.commit_incoming(upload_id, &key).await?;
        let new_version = NewVersion {
            id: self.tokens.generate(16),
            owner: user.id.clone(),
            meta: upload.meta.clone(),
            sha256,
            storage_key: key.clone(),
            uploaded_at: self.clock.now(),
        };
        match self.repo.complete_upload(upload_id, &new_version).await {
            Ok(version) => Ok(version),
            Err(error) => {
                // Another upload may own that key: only clean up our own failures.
                if !matches!(error, AppError::ModIdTaken | AppError::VersionExists) {
                    let _ = self.storage.delete(&key).await;
                }
                Err(error)
            }
        }
    }

    /// Aborts the upload and deletes its bytes.
    pub async fn abort(&self, user: &User, upload_id: &str) -> PortResult<()> {
        let upload = self.owned_upload(user, upload_id).await?;
        self.discard(&upload).await
    }

    /// Removes the session, its incoming bytes and its pending screenshots.
    async fn discard(&self, upload: &ModUpload) -> PortResult<()> {
        self.storage.delete_incoming(&upload.id).await?;
        for screenshot in self.repo.upload_screenshots(&upload.id).await? {
            self.storage.delete(&screenshot.storage_key).await?;
        }
        self.repo.delete_upload(&upload.id).await
    }

    /// Purges sessions past their lifetime together with their bytes.
    pub async fn purge_expired_uploads(&self) -> PortResult<usize> {
        let expired = self.repo.expired_uploads(self.clock.now()).await?;
        for upload in &expired {
            self.discard(upload).await?;
        }
        Ok(expired.len())
    }

    /// The user's mods, each with every version and its review status.
    pub async fn my_mods(&self, user: &User) -> PortResult<Vec<MyMod>> {
        let mut mods: Vec<MyMod> = Vec::new();
        for version in self.repo.versions_of_user(&user.id).await? {
            match mods.iter_mut().find(|m| m.mod_id == version.mod_id) {
                Some(existing) => existing.versions.push(version),
                None => mods.push(MyMod {
                    mod_id: version.mod_id.clone(),
                    versions: vec![version],
                }),
            }
        }
        Ok(mods)
    }

    pub async fn list_pending(&self, admin: &User) -> PortResult<Vec<ModVersion>> {
        self.require_admin(admin)?;
        self.repo.pending_versions().await
    }

    async fn pending_version(&self, mod_id: &str, version: &str) -> PortResult<ModVersion> {
        let slug = ModSlug::parse(mod_id).ok_or(AppError::NotFound)?;
        let found = self
            .repo
            .find_version(&slug, version)
            .await?
            .ok_or(AppError::NotFound)?;
        if found.status != VersionStatus::Pending {
            return Err(AppError::BadRequest(
                "Only a version waiting for review can be reviewed.".into(),
            ));
        }
        Ok(found)
    }

    /// Approves a pending version and publishes its files.
    pub async fn approve(&self, admin: &User, mod_id: &str, version: &str) -> PortResult<()> {
        self.require_admin(admin)?;
        let found = self.pending_version(mod_id, version).await?;
        self.storage.publish(&found.storage_key).await?;
        for screenshot in &found.screenshots {
            self.storage.publish(&screenshot.storage_key).await?;
        }
        self.repo
            .review(
                &found.id,
                VersionStatus::Approved,
                None,
                &admin.id,
                self.clock.now(),
            )
            .await
    }

    /// Rejects a pending version: its files are deleted, the row stays with the reason.
    pub async fn reject(
        &self,
        admin: &User,
        mod_id: &str,
        version: &str,
        reason: &str,
    ) -> PortResult<()> {
        self.require_admin(admin)?;
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_REASON_CHARS {
            return Err(AppError::BadRequest(format!(
                "The reason is required and at most {MAX_REASON_CHARS} characters."
            )));
        }
        let found = self.pending_version(mod_id, version).await?;
        self.storage.delete(&found.storage_key).await?;
        for screenshot in &found.screenshots {
            self.storage.delete(&screenshot.storage_key).await?;
        }
        self.repo
            .review(
                &found.id,
                VersionStatus::Rejected,
                Some(reason),
                &admin.id,
                self.clock.now(),
            )
            .await
    }

    /// The latest approved version of every mod.
    pub async fn public_catalog(&self) -> PortResult<Vec<PublishedMod>> {
        self.repo.latest_approved().await
    }
}

#[cfg(test)]
mod tests;
