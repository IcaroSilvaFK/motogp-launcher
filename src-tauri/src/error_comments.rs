//! Tauri-boundary mapping for the mod comment errors: stable codes plus a
//! user-facing English message. The token and server details never leak into
//! the message.
//!
//! `comment_rate_limited` carries the wait in its message as
//! `... Try again in <N> seconds.`; the frontend reads `<N>` from there
//! (`retryAfterSecs`), because `CommandError` has no extra fields.
use crate::CommandError;
use motogp_application::CommentsError;

/// Shown for every refused write of a banned account (comments, stars, publish).
pub(crate) const BANNED_CODE: &str = "community_banned";
pub(crate) const BANNED_MESSAGE: &str =
    "Your community account is suspended, so you can't post, star or publish.";

impl From<&CommentsError> for CommandError {
    fn from(error: &CommentsError) -> Self {
        let (code, message) = match error {
            CommentsError::Offline => (
                "community_offline",
                "The community is offline. Try again later.".to_string(),
            ),
            CommentsError::SignedOut => (
                "community_signed_out",
                "Sign in to the community to do that.".to_string(),
            ),
            CommentsError::RateLimited { retry_after_secs } => (
                "comment_rate_limited",
                format!("You are commenting too fast. Try again in {retry_after_secs} seconds."),
            ),
            CommentsError::Invalid => (
                "comment_invalid",
                "The comment is empty, too long or has unsupported characters.".to_string(),
            ),
            CommentsError::NotFound => (
                "comment_not_found",
                "That comment no longer exists or is not yours.".to_string(),
            ),
            CommentsError::Banned => (BANNED_CODE, BANNED_MESSAGE.to_string()),
            CommentsError::Credential(_) => (
                "community_signed_out",
                "The saved community sign-in could not be accessed.".to_string(),
            ),
        };
        CommandError {
            code: code.to_string(),
            message,
        }
    }
}

impl From<CommentsError> for CommandError {
    fn from(error: CommentsError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::CredentialError;

    fn mapped(error: CommentsError) -> CommandError {
        CommandError::from(error)
    }

    #[test]
    fn each_error_has_a_stable_code() {
        assert_eq!(mapped(CommentsError::Offline).code, "community_offline");
        assert_eq!(
            mapped(CommentsError::SignedOut).code,
            "community_signed_out"
        );
        assert_eq!(
            mapped(CommentsError::RateLimited {
                retry_after_secs: 3
            })
            .code,
            "comment_rate_limited"
        );
        assert_eq!(mapped(CommentsError::Invalid).code, "comment_invalid");
        assert_eq!(mapped(CommentsError::NotFound).code, "comment_not_found");
        let banned = mapped(CommentsError::Banned);
        assert_eq!(banned.code, "community_banned");
        assert_eq!(
            banned.message,
            "Your community account is suspended, so you can't post, star or publish."
        );
        assert_eq!(
            mapped(CommentsError::Credential(CredentialError("x".into()))).code,
            "community_signed_out"
        );
    }

    #[test]
    fn the_rate_limit_message_carries_the_wait_in_seconds() {
        let error = mapped(CommentsError::RateLimited {
            retry_after_secs: 42,
        });
        assert!(error.message.ends_with("Try again in 42 seconds."));
    }

    #[test]
    fn credential_details_do_not_leak() {
        let error = mapped(CommentsError::Credential(CredentialError(
            "secret-detail".into(),
        )));
        assert!(!error.message.contains("secret-detail"));
    }
}
