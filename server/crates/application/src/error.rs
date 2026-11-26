use std::fmt;

/// Error shared by every use case. The API maps it to the JSON error format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    BadRequest(String),
    InvalidRedirect,
    InvalidCode,
    Unauthorized,
    NotFound,
    RateLimited {
        retry_after_secs: u64,
    },
    DiscordUnavailable,
    /// Mod hosting: the caller may not do this (not an admin, not the owner).
    Forbidden,
    /// Mod hosting: the mod id belongs to another user.
    ModIdTaken,
    /// Mod hosting: a non-rejected version with this number exists.
    VersionExists,
    /// Mod hosting: the chunk offset is not the number of bytes received.
    OffsetMismatch {
        received: u64,
    },
    /// Mod hosting: the file is over the size cap.
    FileTooLarge,
    /// Mod hosting: the per-user storage quota would be exceeded.
    QuotaExceeded,
    /// Mod hosting: file extension or image type not allowed.
    UnsupportedFile(String),
    /// Mod hosting: the server-side SHA-256 differs from the client's.
    ChecksumMismatch,
    /// Mod hosting: completing before all bytes were received.
    UploadIncomplete,
    /// Internal detail: logged, never returned to the client.
    Internal(String),
}

impl AppError {
    /// Stable snake_case code used in the JSON body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::InvalidRedirect => "invalid_redirect",
            Self::InvalidCode => "invalid_code",
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not_found",
            Self::RateLimited { .. } => "rate_limited",
            Self::DiscordUnavailable => "discord_unavailable",
            Self::Forbidden => "forbidden",
            Self::ModIdTaken => "mod_id_taken",
            Self::VersionExists => "version_exists",
            Self::OffsetMismatch { .. } => "offset_mismatch",
            Self::FileTooLarge => "file_too_large",
            Self::QuotaExceeded => "quota_exceeded",
            Self::UnsupportedFile(_) => "unsupported_file",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::UploadIncomplete => "upload_incomplete",
            Self::Internal(_) => "internal",
        }
    }

    /// English message that is safe to show to clients.
    pub fn public_message(&self) -> String {
        match self {
            Self::BadRequest(message) => message.clone(),
            Self::InvalidRedirect => "The redirect URL is not allowed.".into(),
            Self::InvalidCode => "The code is invalid, expired or already used.".into(),
            Self::Unauthorized => "Authentication is required.".into(),
            Self::NotFound => "The resource was not found.".into(),
            Self::RateLimited { .. } => "Too many requests. Try again later.".into(),
            Self::DiscordUnavailable => "Discord is unavailable. Try again later.".into(),
            Self::Forbidden => "You are not allowed to do this.".into(),
            Self::ModIdTaken => "This mod id belongs to another creator.".into(),
            Self::VersionExists => "This version already exists for the mod.".into(),
            Self::OffsetMismatch { .. } => {
                "The chunk offset does not match the bytes already received.".into()
            }
            Self::FileTooLarge => "The file is larger than the allowed size.".into(),
            Self::QuotaExceeded => "The storage quota would be exceeded.".into(),
            Self::UnsupportedFile(message) => message.clone(),
            Self::ChecksumMismatch => "The SHA-256 of the uploaded file does not match.".into(),
            Self::UploadIncomplete => "Not all bytes of the file were received yet.".into(),
            Self::Internal(_) => "An internal error occurred.".into(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(detail) => write!(f, "internal: {detail}"),
            other => write!(f, "{}: {}", other.code(), other.public_message()),
        }
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(AppError::InvalidRedirect.code(), "invalid_redirect");
        assert_eq!(
            AppError::RateLimited {
                retry_after_secs: 1
            }
            .code(),
            "rate_limited"
        );
        assert_eq!(AppError::DiscordUnavailable.code(), "discord_unavailable");
        let hosting = [
            (AppError::Forbidden, "forbidden"),
            (AppError::ModIdTaken, "mod_id_taken"),
            (AppError::VersionExists, "version_exists"),
            (AppError::OffsetMismatch { received: 1 }, "offset_mismatch"),
            (AppError::FileTooLarge, "file_too_large"),
            (AppError::QuotaExceeded, "quota_exceeded"),
            (AppError::UnsupportedFile("x".into()), "unsupported_file"),
            (AppError::ChecksumMismatch, "checksum_mismatch"),
            (AppError::UploadIncomplete, "upload_incomplete"),
        ];
        for (error, code) in hosting {
            assert_eq!(error.code(), code);
            assert!(!error.public_message().is_empty());
        }
    }

    #[test]
    fn internal_detail_is_not_public() {
        let error = AppError::Internal("disk path /secret".into());
        assert!(!error.public_message().contains("secret"));
    }
}
