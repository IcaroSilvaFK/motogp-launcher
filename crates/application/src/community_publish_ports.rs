//! Ports and value types of publishing a mod to the community server (issue
//! #130). The session token only travels as a call argument: it is never part
//! of any value or error defined here.
use motogp_domain::SessionToken;
use std::fmt;
use std::path::{Path, PathBuf};

/// What the creator fills in. Paths are picked by the user in a native dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishRequest {
    pub mod_id: String,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub version: String,
    /// The mod archive (`.zip`, `.7z` or `.pak`).
    pub file: PathBuf,
    /// Up to 8 images (PNG, JPEG or WebP).
    pub screenshots: Vec<PathBuf>,
    /// The creator states the mod is theirs (or they may share it).
    pub ownership_confirmed: bool,
}

/// `POST /mods/uploads` body, after local validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateUpload {
    pub mod_id: String,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub version: String,
    /// Sanitized to the characters the server accepts.
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub ownership_confirmed: bool,
}

/// A started upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSession {
    pub upload_id: String,
    /// Largest chunk the server accepts, in bytes.
    pub chunk_size: u64,
    /// Bytes the server already holds (0 for a new upload).
    pub received: u64,
}

/// `GET /mods/uploads/{id}`: where to resume from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadStatus {
    pub received: u64,
    pub size: u64,
}

/// Review state of one version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionStatus {
    Pending,
    Approved,
    Rejected,
}

/// One uploaded version of a mod, as the server reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModVersion {
    pub mod_id: String,
    pub version: String,
    pub title: String,
    pub description: String,
    pub long_description: Option<String>,
    pub category: String,
    pub game: String,
    pub file_name: String,
    pub size: u64,
    pub sha256: String,
    pub status: VersionStatus,
    pub rejection_reason: Option<String>,
    pub screenshot_count: u32,
    pub uploaded_at: String,
    pub reviewed_at: Option<String>,
}

/// A mod of the signed-in creator with every version it ever had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MyMod {
    pub id: String,
    pub versions: Vec<ModVersion>,
}

/// Why a call to the server failed, already classified from the status and the
/// server's error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishApiError {
    /// 401: the session is missing, expired or revoked.
    Unauthorized,
    /// DNS, connection, timeout or a dropped stream: worth retrying.
    Network(String),
    /// A 5xx answer: worth retrying.
    Server(String),
    /// 403: not the owner of the mod id or of the upload.
    Forbidden,
    /// 413 `file_too_large`.
    FileTooLarge,
    /// 413 `quota_exceeded`.
    QuotaExceeded,
    /// 429 `rate_limited`.
    RateLimited { retry_after_secs: Option<u64> },
    /// 409 `version_exists`.
    VersionExists,
    /// 409 `mod_id_taken`.
    ModIdTaken,
    /// 400 `unsupported_file`.
    UnsupportedFile,
    /// 400 `checksum_mismatch`.
    ChecksumMismatch,
    /// 409 `offset_mismatch`; `received` is the server's byte count when it said so.
    OffsetMismatch { received: Option<u64> },
    /// 400 `bad_request` and every other refusal of the input.
    BadRequest(String),
}

impl PublishApiError {
    /// A failure that may go away by itself: the chunk is asked again.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PublishApiError::Network(_) | PublishApiError::Server(_)
        )
    }
}

impl fmt::Display for PublishApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PublishApiError::Unauthorized => write!(f, "the session is not valid"),
            PublishApiError::Network(detail) => write!(f, "network error: {detail}"),
            PublishApiError::Server(detail) => write!(f, "server error: {detail}"),
            PublishApiError::Forbidden => write!(f, "forbidden"),
            PublishApiError::FileTooLarge => write!(f, "file too large"),
            PublishApiError::QuotaExceeded => write!(f, "storage quota exceeded"),
            PublishApiError::RateLimited {
                retry_after_secs: Some(secs),
            } => write!(f, "rate limited, retry after {secs} s"),
            PublishApiError::RateLimited {
                retry_after_secs: None,
            } => write!(f, "rate limited"),
            PublishApiError::VersionExists => write!(f, "version already exists"),
            PublishApiError::ModIdTaken => write!(f, "mod id taken"),
            PublishApiError::UnsupportedFile => write!(f, "unsupported file"),
            PublishApiError::ChecksumMismatch => write!(f, "checksum mismatch"),
            PublishApiError::OffsetMismatch { .. } => write!(f, "chunk offset mismatch"),
            PublishApiError::BadRequest(detail) => write!(f, "bad request: {detail}"),
        }
    }
}

impl std::error::Error for PublishApiError {}

/// Port: the community server's mod hosting endpoints.
pub trait ModPublishApi: Send + Sync {
    fn create_upload(
        &self,
        token: &SessionToken,
        upload: &CreateUpload,
    ) -> Result<UploadSession, PublishApiError>;

    /// Sends `bytes` at `offset`. Returns the bytes the server now holds.
    fn put_chunk(
        &self,
        token: &SessionToken,
        upload_id: &str,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, PublishApiError>;

    fn upload_status(
        &self,
        token: &SessionToken,
        upload_id: &str,
    ) -> Result<UploadStatus, PublishApiError>;

    /// `content_type` is `image/png`, `image/jpeg` or `image/webp`.
    fn add_screenshot(
        &self,
        token: &SessionToken,
        upload_id: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), PublishApiError>;

    fn complete(
        &self,
        token: &SessionToken,
        upload_id: &str,
    ) -> Result<ModVersion, PublishApiError>;

    fn abort(&self, token: &SessionToken, upload_id: &str) -> Result<(), PublishApiError>;

    fn my_mods(&self, token: &SessionToken) -> Result<Vec<MyMod>, PublishApiError>;
}

/// Port: reads the local files of a publish. Errors are short English texts
/// (they never reach the server).
pub trait PublishFiles: Send + Sync {
    /// Size of the file in bytes.
    fn size(&self, path: &Path) -> Result<u64, String>;
    /// SHA-256 as lowercase hex. `progress` receives the bytes hashed so far.
    fn sha256_hex(&self, path: &Path, progress: &mut dyn FnMut(u64)) -> Result<String, String>;
    /// Up to `len` bytes starting at `offset` (fewer only at the end of the file).
    fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, String>;
}
