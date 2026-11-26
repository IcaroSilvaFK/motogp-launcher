//! Use case: publish a mod to the community server and list the creator's mods
//! (issue #130). The service loads the session token from the credential store
//! itself, so the token never reaches the interface layer.
//!
//! The upload is resumable: the file goes in chunks of the size the server
//! chose, and after a network error the service asks the server how many bytes
//! it holds and continues from there.
use crate::{
    CancelFlag, CreateUpload, CredentialError, CredentialStore, ModPublishApi, ModVersion, MyMod,
    PublishApiError, PublishFiles, PublishRequest,
};
use motogp_domain::SessionToken;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Largest mod file the server accepts.
pub const MAX_MOD_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Screenshots per mod.
pub const MAX_SCREENSHOTS: usize = 8;
/// Largest screenshot.
pub const MAX_SCREENSHOT_BYTES: u64 = 5 * 1024 * 1024;
/// Retries of one chunk after a network or server error.
pub const MAX_CHUNK_RETRIES: u32 = 3;

const MAX_FILE_NAME_LEN: usize = 128;
const RESERVED_IDS: [&str; 3] = ["uploads", "mine", "stats"];

/// Why a publish (or the list of own mods) did not go through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    /// There is no stored session.
    NotSignedIn,
    /// The server rejected the session: sign in again.
    SessionExpired,
    /// The server cannot be reached.
    Offline,
    /// The creator is not allowed (the mod id or the upload is somebody else's).
    Forbidden,
    FileTooLarge,
    QuotaExceeded,
    RateLimited {
        retry_after_secs: Option<u64>,
    },
    VersionExists,
    ModIdTaken,
    UnsupportedFile,
    ChecksumMismatch,
    /// The creator cancelled; the upload was aborted on the server.
    Cancelled,
    /// The request is invalid (found locally, or refused by the server).
    InvalidRequest(String),
    /// The server failed or answered something unexpected.
    Server(String),
    /// The credential store failed.
    Credential(CredentialError),
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PublishError::NotSignedIn => write!(f, "sign in to the community first"),
            PublishError::SessionExpired => write!(f, "the community session expired"),
            PublishError::Offline => write!(f, "the community server is offline"),
            PublishError::Forbidden => write!(f, "not allowed to publish this mod"),
            PublishError::FileTooLarge => write!(f, "the file is too large"),
            PublishError::QuotaExceeded => write!(f, "the storage quota is exceeded"),
            PublishError::RateLimited {
                retry_after_secs: Some(secs),
            } => write!(f, "too many uploads, retry after {secs} s"),
            PublishError::RateLimited {
                retry_after_secs: None,
            } => write!(f, "too many uploads"),
            PublishError::VersionExists => write!(f, "this version already exists"),
            PublishError::ModIdTaken => write!(f, "this mod id belongs to another creator"),
            PublishError::UnsupportedFile => write!(f, "unsupported file type"),
            PublishError::ChecksumMismatch => write!(f, "the uploaded file is corrupted"),
            PublishError::Cancelled => write!(f, "the upload was cancelled"),
            PublishError::InvalidRequest(detail) => write!(f, "invalid request: {detail}"),
            PublishError::Server(detail) => write!(f, "server error: {detail}"),
            PublishError::Credential(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PublishError {}

impl From<CredentialError> for PublishError {
    fn from(error: CredentialError) -> Self {
        PublishError::Credential(error)
    }
}

impl From<PublishApiError> for PublishError {
    fn from(error: PublishApiError) -> Self {
        match error {
            PublishApiError::Unauthorized => PublishError::SessionExpired,
            PublishApiError::Network(_) => PublishError::Offline,
            PublishApiError::Server(detail) => PublishError::Server(detail),
            PublishApiError::Forbidden => PublishError::Forbidden,
            PublishApiError::FileTooLarge => PublishError::FileTooLarge,
            PublishApiError::QuotaExceeded => PublishError::QuotaExceeded,
            PublishApiError::RateLimited { retry_after_secs } => {
                PublishError::RateLimited { retry_after_secs }
            }
            PublishApiError::VersionExists => PublishError::VersionExists,
            PublishApiError::ModIdTaken => PublishError::ModIdTaken,
            PublishApiError::UnsupportedFile => PublishError::UnsupportedFile,
            PublishApiError::ChecksumMismatch => PublishError::ChecksumMismatch,
            PublishApiError::OffsetMismatch { .. } => {
                PublishError::Server("the server lost track of the upload".to_string())
            }
            PublishApiError::BadRequest(detail) => PublishError::InvalidRequest(detail),
        }
    }
}

/// Stage of a publish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishPhase {
    /// Computing the SHA-256 of the file.
    Hashing,
    /// Sending the file chunks.
    Uploading,
    /// Sending the screenshots and completing the upload.
    Finishing,
    Done,
}

/// Progress of a publish. In `Hashing` and `Uploading`, `sent` is bytes of the
/// file; the other phases report `total` of `total`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishProgress {
    pub phase: PublishPhase,
    pub sent: u64,
    pub total: u64,
}

pub struct CommunityPublishService {
    api: Arc<dyn ModPublishApi>,
    files: Arc<dyn PublishFiles>,
    store: Arc<dyn CredentialStore>,
    retry_delay: Duration,
}

impl CommunityPublishService {
    pub fn new(
        api: Arc<dyn ModPublishApi>,
        files: Arc<dyn PublishFiles>,
        store: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            api,
            files,
            store,
            retry_delay: Duration::from_millis(500),
        }
    }

    /// Pause before a chunk retry grows with the retry number (tests use zero).
    pub fn with_retry_delay(self, retry_delay: Duration) -> Self {
        Self {
            retry_delay,
            ..self
        }
    }

    /// The signed-in creator's mods, every version with its review status.
    pub fn my_mods(&self) -> Result<Vec<MyMod>, PublishError> {
        let token = self.token()?;
        Ok(self.api.my_mods(&token)?)
    }

    /// Uploads the mod and leaves it waiting for review. Blocking: run it off
    /// the async runtime. `cancel` aborts the upload on the server.
    pub fn publish(
        &self,
        request: &PublishRequest,
        progress: &mut dyn FnMut(PublishProgress),
        cancel: &CancelFlag,
    ) -> Result<ModVersion, PublishError> {
        let token = self.token()?;
        let file_name = validate_request(request)?;
        let size = self
            .files
            .size(&request.file)
            .map_err(|e| invalid(format!("the mod file cannot be read: {e}")))?;
        if size == 0 {
            return Err(invalid("the mod file is empty"));
        }
        if size > MAX_MOD_FILE_BYTES {
            return Err(PublishError::FileTooLarge);
        }

        let mut hashed = |done: u64| {
            progress(PublishProgress {
                phase: PublishPhase::Hashing,
                sent: done,
                total: size,
            })
        };
        let sha256 = self
            .files
            .sha256_hex(&request.file, &mut hashed)
            .map_err(|e| invalid(format!("the mod file cannot be read: {e}")))?;
        if cancel.is_cancelled() {
            return Err(PublishError::Cancelled);
        }

        let session = self.api.create_upload(
            &token,
            &CreateUpload {
                mod_id: request.mod_id.trim().to_string(),
                title: request.title.trim().to_string(),
                description: request.description.trim().to_string(),
                long_description: request
                    .long_description
                    .as_ref()
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty()),
                category: request.category.trim().to_string(),
                game: request.game.trim().to_string(),
                version: request.version.trim().to_string(),
                file_name,
                size,
                sha256,
                ownership_confirmed: request.ownership_confirmed,
            },
        )?;

        let result = self.upload(
            &token,
            &session.upload_id,
            session.chunk_size,
            session.received,
            size,
            request,
            progress,
            cancel,
        );
        if let Err(error) = &result {
            // Best effort: free the bytes on the server. Nothing to do if it fails.
            if !matches!(error, PublishError::SessionExpired) {
                let _ = self.api.abort(&token, &session.upload_id);
            }
        }
        let version = result?;
        progress(PublishProgress {
            phase: PublishPhase::Done,
            sent: size,
            total: size,
        });
        Ok(version)
    }

    #[allow(clippy::too_many_arguments)]
    fn upload(
        &self,
        token: &SessionToken,
        upload_id: &str,
        chunk_size: u64,
        received: u64,
        size: u64,
        request: &PublishRequest,
        progress: &mut dyn FnMut(PublishProgress),
        cancel: &CancelFlag,
    ) -> Result<ModVersion, PublishError> {
        let chunk_size = chunk_size.max(1);
        let mut offset = received.min(size);
        let mut failures = 0u32;
        let mut report = |phase, sent| {
            progress(PublishProgress {
                phase,
                sent,
                total: size,
            })
        };
        report(PublishPhase::Uploading, offset);
        while offset < size {
            if cancel.is_cancelled() {
                return Err(PublishError::Cancelled);
            }
            let len = chunk_size.min(size - offset);
            let bytes = self
                .files
                .read_range(&request.file, offset, len)
                .map_err(|e| invalid(format!("the mod file cannot be read: {e}")))?;
            match self.api.put_chunk(token, upload_id, offset, &bytes) {
                Ok(now) => {
                    failures = 0;
                    offset = now.min(size);
                    report(PublishPhase::Uploading, offset);
                }
                Err(error) => {
                    failures += 1;
                    if failures > MAX_CHUNK_RETRIES {
                        return Err(error.into());
                    }
                    match error {
                        PublishApiError::OffsetMismatch {
                            received: Some(now),
                        } => offset = now.min(size),
                        error
                            if error.is_retryable()
                                || matches!(error, PublishApiError::OffsetMismatch { .. }) =>
                        {
                            std::thread::sleep(self.retry_delay * failures);
                            // The chunk may or may not have landed: ask the server.
                            match self.api.upload_status(token, upload_id) {
                                Ok(status) => offset = status.received.min(size),
                                Err(status_error) if status_error.is_retryable() => {}
                                Err(status_error) => return Err(status_error.into()),
                            }
                        }
                        error => return Err(error.into()),
                    }
                    report(PublishPhase::Uploading, offset);
                }
            }
        }

        report(PublishPhase::Finishing, size);
        for path in &request.screenshots {
            if cancel.is_cancelled() {
                return Err(PublishError::Cancelled);
            }
            let content_type = image_content_type(path)
                .ok_or_else(|| invalid("screenshots must be PNG, JPEG or WebP images"))?;
            let shot_size = self
                .files
                .size(path)
                .map_err(|e| invalid(format!("a screenshot cannot be read: {e}")))?;
            if shot_size > MAX_SCREENSHOT_BYTES {
                return Err(invalid("a screenshot is larger than 5 MiB"));
            }
            let bytes = self
                .files
                .read_range(path, 0, shot_size)
                .map_err(|e| invalid(format!("a screenshot cannot be read: {e}")))?;
            self.api
                .add_screenshot(token, upload_id, content_type, &bytes)?;
        }
        if cancel.is_cancelled() {
            return Err(PublishError::Cancelled);
        }
        Ok(self.api.complete(token, upload_id)?)
    }

    fn token(&self) -> Result<SessionToken, PublishError> {
        self.store.load()?.ok_or(PublishError::NotSignedIn)
    }
}

fn invalid(detail: impl Into<String>) -> PublishError {
    PublishError::InvalidRequest(detail.into())
}

fn extension(name: &str) -> Option<String> {
    let (_, ext) = name.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

fn image_content_type(path: &std::path::Path) -> Option<&'static str> {
    match extension(path.file_name()?.to_str()?)?.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn is_slug(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

fn check_text(value: &str, what: &str, max: usize) -> Result<(), PublishError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(invalid(format!("{what} is required")));
    }
    if value.chars().count() > max {
        return Err(invalid(format!("{what} is longer than {max} characters")));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid(format!("{what} has invalid characters")));
    }
    Ok(())
}

/// Local checks that mirror the server, so most mistakes never cost an upload.
/// Returns the sanitized file name to send.
fn validate_request(request: &PublishRequest) -> Result<String, PublishError> {
    let id = request.mod_id.trim();
    if !is_slug(id) || RESERVED_IDS.contains(&id) {
        return Err(invalid(
            "the mod id must be lowercase letters, digits and single dashes (at most 64)",
        ));
    }
    check_text(&request.title, "the title", 120)?;
    check_text(&request.description, "the description", 1000)?;
    check_text(&request.category, "the category", 64)?;
    check_text(&request.game, "the game", 64)?;
    if let Some(long) = &request.long_description {
        if long.chars().count() > 20_000 {
            return Err(invalid(
                "the long description is longer than 20000 characters",
            ));
        }
    }
    let version = request.version.trim();
    if version.is_empty()
        || version.len() > 32
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(invalid(
            "the version must be 1 to 32 letters, digits, dots, dashes, underscores or plus signs",
        ));
    }
    if !request.ownership_confirmed {
        return Err(invalid("confirm that you may share this mod"));
    }
    if request.screenshots.len() > MAX_SCREENSHOTS {
        return Err(invalid("at most 8 screenshots"));
    }
    if request
        .screenshots
        .iter()
        .any(|path| image_content_type(path).is_none())
    {
        return Err(invalid("screenshots must be PNG, JPEG or WebP images"));
    }
    let name = request
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("the mod file has no usable name"))?;
    match extension(name).as_deref() {
        Some("zip" | "7z" | "pak") => {}
        _ => return Err(PublishError::UnsupportedFile),
    }
    Ok(sanitize_file_name(name))
}

/// ASCII replacement of a Latin letter with a diacritic.
fn strip_accent(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
        'ç' => "c",
        'Ç' => "C",
        'è' | 'é' | 'ê' | 'ë' => "e",
        'È' | 'É' | 'Ê' | 'Ë' => "E",
        'ì' | 'í' | 'î' | 'ï' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' => "I",
        'ñ' => "n",
        'Ñ' => "N",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => "O",
        'ù' | 'ú' | 'û' | 'ü' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' => "U",
        'ý' | 'ÿ' => "y",
        'Ý' => "Y",
        'ß' => "ss",
        _ => return None,
    })
}

/// Keeps the characters the server allows (letters, digits, space, `.`, `_`,
/// `-`, `(`, `)`); accented letters lose the accent, anything else becomes `_`.
/// The extension is kept and the result is at most 128 bytes.
pub fn sanitize_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '(' | ')') {
            out.push(c);
        } else if let Some(plain) = strip_accent(c) {
            out.push_str(plain);
        } else {
            out.push('_');
        }
    }
    // Neither a hidden name nor leading/trailing blanks.
    let trimmed = out.trim();
    let (stem, ext) = match trimmed.rsplit_once('.') {
        Some((stem, ext)) => (stem, format!(".{ext}")),
        None => (trimmed, String::new()),
    };
    let stem = stem.trim().trim_start_matches('.');
    let stem = if stem.is_empty() { "mod" } else { stem };
    let room = MAX_FILE_NAME_LEN.saturating_sub(ext.len());
    let stem: String = stem.chars().take(room).collect();
    format!("{stem}{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PublishPhase, UploadSession, UploadStatus, VersionStatus};
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    const TOKEN: &str = "tok-SECRET";

    struct Store(Option<&'static str>);

    impl CredentialStore for Store {
        fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
            Ok(self.0.map(SessionToken::new))
        }
        fn save(&self, _token: &SessionToken) -> Result<(), CredentialError> {
            Ok(())
        }
        fn delete(&self) -> Result<(), CredentialError> {
            Ok(())
        }
    }

    struct BrokenStore;

    impl CredentialStore for BrokenStore {
        fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
            Err(CredentialError("locked".into()))
        }
        fn save(&self, _token: &SessionToken) -> Result<(), CredentialError> {
            Ok(())
        }
        fn delete(&self) -> Result<(), CredentialError> {
            Ok(())
        }
    }

    /// An in-memory file system: path name to content.
    struct FakeFiles {
        files: Vec<(&'static str, Vec<u8>)>,
        hash: String,
    }

    impl FakeFiles {
        fn get(&self, path: &Path) -> Result<&Vec<u8>, String> {
            self.files
                .iter()
                .find(|(name, _)| path.ends_with(name))
                .map(|(_, data)| data)
                .ok_or_else(|| "missing".to_string())
        }
    }

    impl PublishFiles for FakeFiles {
        fn size(&self, path: &Path) -> Result<u64, String> {
            Ok(self.get(path)?.len() as u64)
        }
        fn sha256_hex(&self, path: &Path, progress: &mut dyn FnMut(u64)) -> Result<String, String> {
            let len = self.get(path)?.len() as u64;
            progress(len);
            Ok(self.hash.clone())
        }
        fn read_range(&self, path: &Path, offset: u64, len: u64) -> Result<Vec<u8>, String> {
            let data = self.get(path)?;
            let start = (offset as usize).min(data.len());
            let end = ((offset + len) as usize).min(data.len());
            Ok(data[start..end].to_vec())
        }
    }

    #[derive(Debug, PartialEq, Eq, Clone)]
    enum Call {
        Create(CreateUpload),
        Chunk(u64, usize),
        Status,
        Shot(String, usize),
        Complete,
        Abort,
        Mine,
    }

    /// Scripted server: `chunks` and `statuses` answer in order (default: succeed).
    struct FakeApi {
        chunk_size: u64,
        initial_received: u64,
        create: Mutex<Option<Result<(), PublishApiError>>>,
        chunks: Mutex<VecDeque<Result<Option<u64>, PublishApiError>>>,
        statuses: Mutex<VecDeque<Result<u64, PublishApiError>>>,
        shot: Mutex<Option<PublishApiError>>,
        complete: Mutex<Option<PublishApiError>>,
        held: Mutex<u64>,
        calls: Mutex<Vec<Call>>,
        tokens: Mutex<Vec<String>>,
        on_chunk: Mutex<Option<Box<dyn Fn() + Send>>>,
    }

    impl FakeApi {
        fn new(chunk_size: u64) -> Self {
            Self {
                chunk_size,
                initial_received: 0,
                create: Mutex::default(),
                chunks: Mutex::default(),
                statuses: Mutex::default(),
                shot: Mutex::default(),
                complete: Mutex::default(),
                held: Mutex::default(),
                calls: Mutex::default(),
                tokens: Mutex::default(),
                on_chunk: Mutex::default(),
            }
        }
        fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }
        fn note(&self, call: Call, token: &SessionToken) {
            self.calls.lock().unwrap().push(call);
            self.tokens.lock().unwrap().push(token.expose().to_string());
        }
    }

    fn version() -> ModVersion {
        ModVersion {
            mod_id: "brazil-livery".into(),
            version: "1.0".into(),
            title: "Brazil Livery".into(),
            description: "d".into(),
            long_description: None,
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            file_name: "Brazil Livery.zip".into(),
            size: 10,
            sha256: "h".into(),
            status: VersionStatus::Pending,
            rejection_reason: None,
            screenshot_count: 0,
            uploaded_at: "2026-11-26T10:00:00Z".into(),
            reviewed_at: None,
        }
    }

    impl ModPublishApi for FakeApi {
        fn create_upload(
            &self,
            token: &SessionToken,
            upload: &CreateUpload,
        ) -> Result<UploadSession, PublishApiError> {
            self.note(Call::Create(upload.clone()), token);
            if let Some(result) = self.create.lock().unwrap().take() {
                result?;
            }
            *self.held.lock().unwrap() = self.initial_received;
            Ok(UploadSession {
                upload_id: "up1".into(),
                chunk_size: self.chunk_size,
                received: self.initial_received,
            })
        }
        fn put_chunk(
            &self,
            token: &SessionToken,
            _id: &str,
            offset: u64,
            bytes: &[u8],
        ) -> Result<u64, PublishApiError> {
            self.note(Call::Chunk(offset, bytes.len()), token);
            if let Some(hook) = self.on_chunk.lock().unwrap().as_ref() {
                hook();
            }
            let scripted = self.chunks.lock().unwrap().pop_front();
            match scripted {
                // Err: nothing stored. Ok(Some(n)): the server now holds n bytes
                // (a lost answer is simulated by the status script instead).
                Some(Err(error)) => Err(error),
                Some(Ok(Some(now))) => {
                    *self.held.lock().unwrap() = now;
                    Ok(now)
                }
                Some(Ok(None)) | None => {
                    let now = offset + bytes.len() as u64;
                    *self.held.lock().unwrap() = now;
                    Ok(now)
                }
            }
        }
        fn upload_status(
            &self,
            token: &SessionToken,
            _id: &str,
        ) -> Result<UploadStatus, PublishApiError> {
            self.note(Call::Status, token);
            let received = match self.statuses.lock().unwrap().pop_front() {
                Some(answer) => answer?,
                None => *self.held.lock().unwrap(),
            };
            Ok(UploadStatus { received, size: 0 })
        }
        fn add_screenshot(
            &self,
            token: &SessionToken,
            _id: &str,
            content_type: &str,
            bytes: &[u8],
        ) -> Result<(), PublishApiError> {
            self.note(Call::Shot(content_type.into(), bytes.len()), token);
            match self.shot.lock().unwrap().take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }
        fn complete(&self, token: &SessionToken, _id: &str) -> Result<ModVersion, PublishApiError> {
            self.note(Call::Complete, token);
            match self.complete.lock().unwrap().take() {
                Some(error) => Err(error),
                None => Ok(version()),
            }
        }
        fn abort(&self, token: &SessionToken, _id: &str) -> Result<(), PublishApiError> {
            self.note(Call::Abort, token);
            Ok(())
        }
        fn my_mods(&self, token: &SessionToken) -> Result<Vec<MyMod>, PublishApiError> {
            self.note(Call::Mine, token);
            Ok(vec![MyMod {
                id: "brazil-livery".into(),
                versions: vec![version()],
            }])
        }
    }

    fn request() -> PublishRequest {
        PublishRequest {
            mod_id: "brazil-livery".into(),
            title: "Brazil Livery".into(),
            description: "A livery".into(),
            long_description: Some("  ".into()),
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            version: "1.0".into(),
            file: PathBuf::from("C:/mods/Brazil Livery.zip"),
            screenshots: vec![],
            ownership_confirmed: true,
        }
    }

    fn files(size: usize) -> Arc<FakeFiles> {
        Arc::new(FakeFiles {
            files: vec![
                ("Brazil Livery.zip", (0..size).map(|i| i as u8).collect()),
                ("a.png", vec![1; 5]),
                ("b.jpg", vec![2; 6]),
                ("big.png", vec![0; (MAX_SCREENSHOT_BYTES + 1) as usize]),
            ],
            hash: "ab".repeat(32),
        })
    }

    fn service(api: &Arc<FakeApi>, files: Arc<FakeFiles>) -> CommunityPublishService {
        CommunityPublishService::new(api.clone(), files, Arc::new(Store(Some(TOKEN))))
            .with_retry_delay(Duration::ZERO)
    }

    fn run(
        service: &CommunityPublishService,
        request: &PublishRequest,
    ) -> (Result<ModVersion, PublishError>, Vec<PublishProgress>) {
        let mut seen = Vec::new();
        let result = service.publish(request, &mut |p| seen.push(p), &CancelFlag::new());
        (result, seen)
    }

    #[test]
    fn publishes_in_chunks_and_reports_every_phase() {
        let api = Arc::new(FakeApi::new(4));
        let service = service(&api, files(10));
        let (result, seen) = run(&service, &request());
        assert_eq!(result.unwrap().status, VersionStatus::Pending);
        let calls = api.calls();
        let Call::Create(upload) = &calls[0] else {
            panic!("create first");
        };
        assert_eq!(upload.file_name, "Brazil Livery.zip");
        assert_eq!(upload.size, 10);
        assert_eq!(upload.sha256, "ab".repeat(32));
        assert_eq!(upload.long_description, None);
        assert!(upload.ownership_confirmed);
        assert_eq!(
            calls[1..],
            [
                Call::Chunk(0, 4),
                Call::Chunk(4, 4),
                Call::Chunk(8, 2),
                Call::Complete
            ]
        );
        let phases: Vec<_> = seen.iter().map(|p| p.phase).collect();
        assert_eq!(phases.first(), Some(&PublishPhase::Hashing));
        assert_eq!(phases.last(), Some(&PublishPhase::Done));
        assert!(phases.contains(&PublishPhase::Uploading));
        assert!(phases.contains(&PublishPhase::Finishing));
        let sent: Vec<u64> = seen
            .iter()
            .filter(|p| p.phase == PublishPhase::Uploading)
            .map(|p| p.sent)
            .collect();
        assert_eq!(sent, vec![0, 4, 8, 10]);
        assert!(seen.iter().all(|p| p.total == 10));
        assert!(api.tokens.lock().unwrap().iter().all(|t| t == TOKEN));
    }

    #[test]
    fn screenshots_are_sent_with_their_content_type() {
        let api = Arc::new(FakeApi::new(100));
        let service = service(&api, files(10));
        let mut req = request();
        req.screenshots = vec![PathBuf::from("a.png"), PathBuf::from("b.jpg")];
        run(&service, &req).0.unwrap();
        let calls = api.calls();
        assert!(calls.contains(&Call::Shot("image/png".into(), 5)));
        assert!(calls.contains(&Call::Shot("image/jpeg".into(), 6)));
        assert_eq!(calls.last(), Some(&Call::Complete));
    }

    #[test]
    fn a_signed_out_creator_cannot_publish_or_list() {
        let api = Arc::new(FakeApi::new(4));
        let service = CommunityPublishService::new(api.clone(), files(10), Arc::new(Store(None)));
        assert_eq!(run(&service, &request()).0, Err(PublishError::NotSignedIn));
        assert_eq!(service.my_mods(), Err(PublishError::NotSignedIn));
        assert!(api.calls().is_empty());
    }

    #[test]
    fn a_broken_credential_store_is_reported() {
        let api = Arc::new(FakeApi::new(4));
        let service = CommunityPublishService::new(api, files(10), Arc::new(BrokenStore));
        assert!(matches!(
            service.my_mods(),
            Err(PublishError::Credential(_))
        ));
    }

    #[test]
    fn my_mods_lists_the_versions() {
        let api = Arc::new(FakeApi::new(4));
        let mods = service(&api, files(1)).my_mods().unwrap();
        assert_eq!(mods[0].versions.len(), 1);
        assert_eq!(api.calls(), vec![Call::Mine]);
    }

    #[test]
    fn invalid_requests_never_reach_the_server() {
        type Mutation = Box<dyn Fn(&mut PublishRequest)>;
        let cases: Vec<(Mutation, PublishError)> = vec![
            (
                Box::new(|r| r.mod_id = "Bad Id".into()),
                invalid("the mod id must be lowercase letters, digits and single dashes (at most 64)"),
            ),
            (
                Box::new(|r| r.mod_id = "stats".into()),
                invalid("the mod id must be lowercase letters, digits and single dashes (at most 64)"),
            ),
            (Box::new(|r| r.title = "  ".into()), invalid("the title is required")),
            (
                Box::new(|r| r.description = "d".repeat(1001)),
                invalid("the description is longer than 1000 characters"),
            ),
            (
                Box::new(|r| r.game = "a\u{0}b".into()),
                invalid("the game has invalid characters"),
            ),
            (
                Box::new(|r| r.long_description = Some("x".repeat(20_001))),
                invalid("the long description is longer than 20000 characters"),
            ),
            (
                Box::new(|r| r.version = "../1".into()),
                invalid("the version must be 1 to 32 letters, digits, dots, dashes, underscores or plus signs"),
            ),
            (
                Box::new(|r| r.ownership_confirmed = false),
                invalid("confirm that you may share this mod"),
            ),
            (
                Box::new(|r| r.screenshots = vec![PathBuf::from("a.png"); 9]),
                invalid("at most 8 screenshots"),
            ),
            (
                Box::new(|r| r.screenshots = vec![PathBuf::from("a.gif")]),
                invalid("screenshots must be PNG, JPEG or WebP images"),
            ),
            (
                Box::new(|r| r.file = PathBuf::from("C:/mods/setup.exe")),
                PublishError::UnsupportedFile,
            ),
            (
                Box::new(|r| r.file = PathBuf::from("C:/mods/noextension")),
                PublishError::UnsupportedFile,
            ),
        ];
        for (mutate, expected) in cases {
            let api = Arc::new(FakeApi::new(4));
            let mut req = request();
            mutate(&mut req);
            let (result, _) = run(&service(&api, files(10)), &req);
            assert_eq!(result, Err(expected));
            assert!(api.calls().is_empty());
        }
    }

    #[test]
    fn extensions_are_case_insensitive() {
        let api = Arc::new(FakeApi::new(100));
        let fs = Arc::new(FakeFiles {
            files: vec![("MOD.PAK", vec![1; 3])],
            hash: "00".repeat(32),
        });
        let mut req = request();
        req.file = PathBuf::from("MOD.PAK");
        run(&service(&api, fs), &req).0.unwrap();
    }

    #[test]
    fn file_size_rules_are_checked_before_hashing() {
        let api = Arc::new(FakeApi::new(4));
        let empty = Arc::new(FakeFiles {
            files: vec![("Brazil Livery.zip", vec![])],
            hash: String::new(),
        });
        assert_eq!(
            run(&service(&api, empty), &request()).0,
            Err(invalid("the mod file is empty"))
        );
        // A file the fake cannot find.
        let mut req = request();
        req.file = PathBuf::from("C:/mods/missing.zip");
        assert!(matches!(
            run(&service(&api, files(10)), &req).0,
            Err(PublishError::InvalidRequest(_))
        ));
        assert!(api.calls().is_empty());
    }

    /// A file whose size is declared huge without allocating it.
    struct HugeFiles;

    impl PublishFiles for HugeFiles {
        fn size(&self, _path: &Path) -> Result<u64, String> {
            Ok(MAX_MOD_FILE_BYTES + 1)
        }
        fn sha256_hex(&self, _p: &Path, _f: &mut dyn FnMut(u64)) -> Result<String, String> {
            panic!("must not hash a file that is too large")
        }
        fn read_range(&self, _p: &Path, _o: u64, _l: u64) -> Result<Vec<u8>, String> {
            panic!("must not read")
        }
    }

    #[test]
    fn a_file_over_two_gib_is_refused_locally() {
        let api = Arc::new(FakeApi::new(4));
        let service = CommunityPublishService::new(
            api.clone(),
            Arc::new(HugeFiles),
            Arc::new(Store(Some(TOKEN))),
        );
        let (result, _) = run(&service, &request());
        assert_eq!(result, Err(PublishError::FileTooLarge));
        assert!(api.calls().is_empty());
    }

    #[test]
    fn an_oversized_screenshot_is_refused_and_the_upload_is_aborted() {
        let api = Arc::new(FakeApi::new(100));
        let mut req = request();
        req.screenshots = vec![PathBuf::from("big.png")];
        let (result, _) = run(&service(&api, files(10)), &req);
        assert!(matches!(result, Err(PublishError::InvalidRequest(_))));
        assert_eq!(api.calls().last(), Some(&Call::Abort));
    }

    #[test]
    fn a_network_error_resumes_from_the_bytes_the_server_holds() {
        let api = Arc::new(FakeApi::new(4));
        // The second chunk fails after the server stored it (lost answer).
        api.chunks
            .lock()
            .unwrap()
            .extend([Ok(None), Err(PublishApiError::Network("reset".into()))]);
        api.statuses.lock().unwrap().push_back(Ok(8));
        let (result, _) = run(&service(&api, files(10)), &request());
        result.unwrap();
        let calls = api.calls();
        assert_eq!(
            calls[1..],
            [
                Call::Chunk(0, 4),
                Call::Chunk(4, 4),
                Call::Status,
                Call::Chunk(8, 2),
                Call::Complete
            ]
        );
    }

    #[test]
    fn an_offset_mismatch_resumes_from_the_servers_received_bytes() {
        let api = Arc::new(FakeApi::new(4));
        api.chunks
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::OffsetMismatch { received: Some(6) }));
        let (result, _) = run(&service(&api, files(10)), &request());
        result.unwrap();
        assert_eq!(
            api.calls()[1..],
            [Call::Chunk(0, 4), Call::Chunk(6, 4), Call::Complete]
        );
    }

    #[test]
    fn an_offset_mismatch_without_a_count_asks_the_status() {
        let api = Arc::new(FakeApi::new(4));
        api.chunks
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::OffsetMismatch { received: None }));
        api.statuses.lock().unwrap().push_back(Ok(4));
        run(&service(&api, files(8)), &request()).0.unwrap();
        assert_eq!(
            api.calls()[1..],
            [
                Call::Chunk(0, 4),
                Call::Status,
                Call::Chunk(4, 4),
                Call::Complete
            ]
        );
    }

    #[test]
    fn a_failing_status_call_is_retried_through_the_chunk() {
        let api = Arc::new(FakeApi::new(100));
        api.chunks
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::Server("500".into())));
        api.statuses
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::Network("down".into())));
        run(&service(&api, files(5)), &request()).0.unwrap();
        assert_eq!(
            api.calls()[1..],
            [
                Call::Chunk(0, 5),
                Call::Status,
                Call::Chunk(0, 5),
                Call::Complete
            ]
        );
    }

    #[test]
    fn a_hard_status_error_stops_the_upload() {
        let api = Arc::new(FakeApi::new(100));
        api.chunks
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::Network("down".into())));
        api.statuses
            .lock()
            .unwrap()
            .push_back(Err(PublishApiError::Unauthorized));
        let (result, _) = run(&service(&api, files(5)), &request());
        assert_eq!(result, Err(PublishError::SessionExpired));
    }

    #[test]
    fn a_chunk_gives_up_after_three_retries() {
        let api = Arc::new(FakeApi::new(100));
        api.chunks
            .lock()
            .unwrap()
            .extend((0..10).map(|_| Err(PublishApiError::Network("down".into()))));
        let (result, _) = run(&service(&api, files(5)), &request());
        assert_eq!(result, Err(PublishError::Offline));
        let chunks = api
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Chunk(..)))
            .count();
        assert_eq!(chunks, 4); // the first try and three retries
        assert_eq!(api.calls().last(), Some(&Call::Abort));
    }

    #[test]
    fn the_retry_budget_is_per_chunk() {
        let api = Arc::new(FakeApi::new(4));
        // Two failures on each of two chunks: fine, the counter resets on success.
        api.chunks.lock().unwrap().extend([
            Err(PublishApiError::Network("x".into())),
            Err(PublishApiError::Network("x".into())),
            Ok(None),
            Err(PublishApiError::Network("x".into())),
            Err(PublishApiError::Network("x".into())),
        ]);
        run(&service(&api, files(8)), &request()).0.unwrap();
    }

    #[test]
    fn a_repeated_offset_mismatch_gives_up() {
        let api = Arc::new(FakeApi::new(4));
        api.chunks
            .lock()
            .unwrap()
            .extend((0..10).map(|_| Err(PublishApiError::OffsetMismatch { received: Some(1) })));
        let (result, _) = run(&service(&api, files(8)), &request());
        assert!(matches!(result, Err(PublishError::Server(_))));
    }

    #[test]
    fn a_resumed_session_starts_from_what_the_server_has() {
        let mut fake = FakeApi::new(4);
        fake.initial_received = 4;
        let api = Arc::new(fake);
        run(&service(&api, files(10)), &request()).0.unwrap();
        assert_eq!(api.calls()[1], Call::Chunk(4, 4));
    }

    #[test]
    fn server_refusals_map_to_publish_errors() {
        let cases = [
            (PublishApiError::FileTooLarge, PublishError::FileTooLarge),
            (PublishApiError::QuotaExceeded, PublishError::QuotaExceeded),
            (PublishApiError::VersionExists, PublishError::VersionExists),
            (PublishApiError::ModIdTaken, PublishError::ModIdTaken),
            (
                PublishApiError::UnsupportedFile,
                PublishError::UnsupportedFile,
            ),
            (PublishApiError::Forbidden, PublishError::Forbidden),
            (PublishApiError::Unauthorized, PublishError::SessionExpired),
            (
                PublishApiError::RateLimited {
                    retry_after_secs: Some(60),
                },
                PublishError::RateLimited {
                    retry_after_secs: Some(60),
                },
            ),
            (
                PublishApiError::BadRequest("x".into()),
                PublishError::InvalidRequest("x".into()),
            ),
            (PublishApiError::Network("n".into()), PublishError::Offline),
        ];
        for (api_error, expected) in cases {
            let api = Arc::new(FakeApi::new(4));
            *api.create.lock().unwrap() = Some(Err(api_error.clone()));
            let (result, _) = run(&service(&api, files(10)), &request());
            assert_eq!(result, Err(expected), "{api_error}");
        }
    }

    #[test]
    fn a_failed_create_does_not_abort() {
        let api = Arc::new(FakeApi::new(4));
        *api.create.lock().unwrap() = Some(Err(PublishApiError::FileTooLarge));
        run(&service(&api, files(10)), &request()).0.unwrap_err();
        assert_eq!(api.calls().len(), 1);
    }

    #[test]
    fn a_checksum_mismatch_on_complete_aborts_and_maps() {
        let api = Arc::new(FakeApi::new(100));
        *api.complete.lock().unwrap() = Some(PublishApiError::ChecksumMismatch);
        let (result, _) = run(&service(&api, files(5)), &request());
        assert_eq!(result, Err(PublishError::ChecksumMismatch));
        assert_eq!(api.calls().last(), Some(&Call::Abort));
    }

    #[test]
    fn an_expired_session_mid_upload_is_not_aborted() {
        let api = Arc::new(FakeApi::new(100));
        *api.complete.lock().unwrap() = Some(PublishApiError::Unauthorized);
        let (result, _) = run(&service(&api, files(5)), &request());
        assert_eq!(result, Err(PublishError::SessionExpired));
        assert!(!api.calls().contains(&Call::Abort));
    }

    #[test]
    fn a_screenshot_error_aborts_the_upload() {
        let api = Arc::new(FakeApi::new(100));
        *api.shot.lock().unwrap() = Some(PublishApiError::UnsupportedFile);
        let mut req = request();
        req.screenshots = vec![PathBuf::from("a.png")];
        let (result, _) = run(&service(&api, files(5)), &req);
        assert_eq!(result, Err(PublishError::UnsupportedFile));
        assert_eq!(api.calls().last(), Some(&Call::Abort));
    }

    #[test]
    fn cancelling_before_the_upload_starts_sends_nothing() {
        let api = Arc::new(FakeApi::new(4));
        let cancel = CancelFlag::new();
        cancel.cancel();
        let result = service(&api, files(10)).publish(&request(), &mut |_| {}, &cancel);
        assert_eq!(result, Err(PublishError::Cancelled));
        assert!(api.calls().is_empty());
    }

    #[test]
    fn cancelling_mid_upload_aborts_on_the_server() {
        let api = Arc::new(FakeApi::new(4));
        let cancel = CancelFlag::new();
        let flag = cancel.clone();
        *api.on_chunk.lock().unwrap() = Some(Box::new(move || flag.cancel()));
        let result = service(&api, files(10)).publish(&request(), &mut |_| {}, &cancel);
        assert_eq!(result, Err(PublishError::Cancelled));
        assert_eq!(api.calls()[1..], [Call::Chunk(0, 4), Call::Abort]);
    }

    #[test]
    fn cancelling_before_the_screenshots_or_complete_aborts() {
        let api = Arc::new(FakeApi::new(100));
        let cancel = CancelFlag::new();
        let flag = cancel.clone();
        *api.on_chunk.lock().unwrap() = Some(Box::new(move || flag.cancel()));
        let mut req = request();
        req.screenshots = vec![PathBuf::from("a.png")];
        let result = service(&api, files(5)).publish(&req, &mut |_| {}, &cancel);
        assert_eq!(result, Err(PublishError::Cancelled));
        assert_eq!(api.calls().last(), Some(&Call::Abort));
        assert!(!api.calls().iter().any(|c| matches!(c, Call::Shot(..))));
        // And with no screenshots: the check before `complete` fires.
        let api = Arc::new(FakeApi::new(100));
        let cancel = CancelFlag::new();
        let flag = cancel.clone();
        *api.on_chunk.lock().unwrap() = Some(Box::new(move || flag.cancel()));
        let result = service(&api, files(5)).publish(&request(), &mut |_| {}, &cancel);
        assert_eq!(result, Err(PublishError::Cancelled));
        assert!(!api.calls().contains(&Call::Complete));
    }

    #[test]
    fn file_names_are_sanitized_to_the_servers_alphabet() {
        assert_eq!(
            sanitize_file_name("Brazil Livery (v2).zip"),
            "Brazil Livery (v2).zip"
        );
        assert_eq!(
            sanitize_file_name("Pintura Açaí São João.7z"),
            "Pintura Acai Sao Joao.7z"
        );
        assert_eq!(sanitize_file_name("a+b&c#d.zip"), "a_b_c_d.zip");
        assert_eq!(sanitize_file_name("日本.pak"), "__.pak");
        assert_eq!(
            sanitize_file_name("ÀÇÈÌÑÒÙÝ àçèìñòùÿ ß.zip"),
            "ACEINOUY aceinouy ss.zip"
        );
        assert_eq!(sanitize_file_name(".zip"), "mod.zip");
        assert_eq!(sanitize_file_name("  .hidden.zip"), "hidden.zip");
        assert_eq!(sanitize_file_name("noext"), "noext");
        let long = format!("{}.zip", "x".repeat(300));
        let name = sanitize_file_name(&long);
        assert_eq!(name.len(), 128);
        assert!(name.ends_with(".zip"));
    }

    #[test]
    fn the_sanitized_name_is_what_the_server_receives() {
        let api = Arc::new(FakeApi::new(100));
        let fs = Arc::new(FakeFiles {
            files: vec![("Pintura Açaí.zip", vec![1; 3])],
            hash: "00".repeat(32),
        });
        let mut req = request();
        req.file = PathBuf::from("C:/Usuário Padrão/Pintura Açaí.zip");
        run(&service(&api, fs), &req).0.unwrap();
        let Call::Create(upload) = &api.calls()[0] else {
            panic!("create first");
        };
        assert_eq!(upload.file_name, "Pintura Acai.zip");
    }

    #[test]
    fn errors_have_readable_messages_and_convert() {
        let all = [
            PublishError::NotSignedIn,
            PublishError::SessionExpired,
            PublishError::Offline,
            PublishError::Forbidden,
            PublishError::FileTooLarge,
            PublishError::QuotaExceeded,
            PublishError::RateLimited {
                retry_after_secs: Some(3),
            },
            PublishError::RateLimited {
                retry_after_secs: None,
            },
            PublishError::VersionExists,
            PublishError::ModIdTaken,
            PublishError::UnsupportedFile,
            PublishError::ChecksumMismatch,
            PublishError::Cancelled,
            PublishError::InvalidRequest("x".into()),
            PublishError::Server("y".into()),
            PublishError::from(CredentialError("locked".into())),
        ];
        assert!(all.iter().all(|e| !e.to_string().is_empty()));
        assert!(all[6].to_string().contains('3'));
        let _: &dyn std::error::Error = &all[0];
        let api_errors = [
            PublishApiError::Unauthorized,
            PublishApiError::Network("n".into()),
            PublishApiError::Server("s".into()),
            PublishApiError::Forbidden,
            PublishApiError::FileTooLarge,
            PublishApiError::QuotaExceeded,
            PublishApiError::RateLimited {
                retry_after_secs: Some(1),
            },
            PublishApiError::RateLimited {
                retry_after_secs: None,
            },
            PublishApiError::VersionExists,
            PublishApiError::ModIdTaken,
            PublishApiError::UnsupportedFile,
            PublishApiError::ChecksumMismatch,
            PublishApiError::OffsetMismatch { received: None },
            PublishApiError::BadRequest("b".into()),
        ];
        for error in api_errors {
            assert!(!error.to_string().is_empty());
            let _: &dyn std::error::Error = &error;
            let _ = PublishError::from(error);
        }
    }
}
