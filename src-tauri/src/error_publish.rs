//! Tauri-boundary mapping for the publish errors (#130): stable codes plus a
//! user-facing English message. The token and server internals never leak.
use crate::CommandError;
use motogp_application::PublishError;

impl From<&PublishError> for CommandError {
    fn from(error: &PublishError) -> Self {
        let (code, message) = match error {
            PublishError::NotSignedIn | PublishError::Credential(_) => (
                "not_signed_in",
                "Sign in to the community to publish a mod.".to_string(),
            ),
            PublishError::SessionExpired => (
                "session_expired",
                "Your community session expired. Sign in again.".to_string(),
            ),
            PublishError::Offline => (
                "offline",
                "The community server cannot be reached. Try again later.".to_string(),
            ),
            PublishError::Forbidden => (
                "forbidden",
                "You are not allowed to publish this mod.".to_string(),
            ),
            PublishError::FileTooLarge => (
                "file_too_large",
                "The mod file is too large (the limit is 2 GiB).".to_string(),
            ),
            PublishError::QuotaExceeded => (
                "quota_exceeded",
                "Your storage quota on the community server is full.".to_string(),
            ),
            PublishError::RateLimited {
                retry_after_secs: Some(secs),
            } => (
                "rate_limited",
                format!("Too many uploads. Try again in {secs} seconds."),
            ),
            PublishError::RateLimited {
                retry_after_secs: None,
            } => (
                "rate_limited",
                "Too many uploads. Try again later.".to_string(),
            ),
            PublishError::VersionExists => (
                "version_exists",
                "This version of the mod was already published. Use a new version number."
                    .to_string(),
            ),
            PublishError::ModIdTaken => (
                "mod_id_taken",
                "This mod id belongs to another creator. Pick a different one.".to_string(),
            ),
            PublishError::UnsupportedFile => (
                "unsupported_file",
                "Only .zip, .7z and .pak files (and PNG, JPEG or WebP images) can be published."
                    .to_string(),
            ),
            PublishError::ChecksumMismatch => (
                "checksum_mismatch",
                "The uploaded file did not match the original. Try again.".to_string(),
            ),
            PublishError::Cancelled => ("cancelled", "The upload was cancelled.".to_string()),
            // The detail is written by the launcher or the server for the user.
            PublishError::InvalidRequest(detail) => ("invalid_request", detail.clone()),
            PublishError::Server(_) => (
                "server_error",
                "The community server failed. Try again later.".to_string(),
            ),
        };
        CommandError {
            code: code.to_string(),
            message,
        }
    }
}

impl From<PublishError> for CommandError {
    fn from(error: PublishError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::CredentialError;

    fn mapped(error: PublishError) -> CommandError {
        CommandError::from(error)
    }

    #[test]
    fn each_error_has_a_stable_code() {
        let cases = [
            (PublishError::NotSignedIn, "not_signed_in"),
            (
                PublishError::Credential(CredentialError("x".into())),
                "not_signed_in",
            ),
            (PublishError::SessionExpired, "session_expired"),
            (PublishError::Offline, "offline"),
            (PublishError::Forbidden, "forbidden"),
            (PublishError::FileTooLarge, "file_too_large"),
            (PublishError::QuotaExceeded, "quota_exceeded"),
            (
                PublishError::RateLimited {
                    retry_after_secs: None,
                },
                "rate_limited",
            ),
            (PublishError::VersionExists, "version_exists"),
            (PublishError::ModIdTaken, "mod_id_taken"),
            (PublishError::UnsupportedFile, "unsupported_file"),
            (PublishError::ChecksumMismatch, "checksum_mismatch"),
            (PublishError::Cancelled, "cancelled"),
            (PublishError::InvalidRequest("x".into()), "invalid_request"),
            (PublishError::Server("x".into()), "server_error"),
        ];
        for (error, code) in cases {
            let mapped = mapped(error);
            assert_eq!(mapped.code, code);
            assert!(!mapped.message.is_empty());
        }
    }

    #[test]
    fn the_retry_delay_is_in_the_message() {
        let error = mapped(PublishError::RateLimited {
            retry_after_secs: Some(90),
        });
        assert_eq!(error.code, "rate_limited");
        assert!(error.message.contains("90"));
    }

    #[test]
    fn an_invalid_request_shows_its_detail_but_other_details_do_not_leak() {
        assert_eq!(
            mapped(PublishError::InvalidRequest("the title is required".into())).message,
            "the title is required"
        );
        assert!(!mapped(PublishError::Server("secret-detail".into()))
            .message
            .contains("secret-detail"));
        assert!(!mapped(PublishError::Credential(CredentialError(
            "secret-detail".into()
        )))
        .message
        .contains("secret-detail"));
    }
}
