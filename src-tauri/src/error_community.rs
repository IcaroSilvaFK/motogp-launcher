//! Tauri-boundary mapping for the community account errors: stable codes plus
//! a user-facing English message. The token, codes and server details never
//! leak into the message.
use crate::error_comments::{BANNED_CODE, BANNED_MESSAGE};
use crate::CommandError;
use motogp_application::CommunityError;

impl From<&CommunityError> for CommandError {
    fn from(error: &CommunityError) -> Self {
        let (code, message) = match error {
            CommunityError::Offline => (
                "community_offline",
                "The community is offline. Try again later.",
            ),
            CommunityError::SignInFailed => (
                "community_sign_in_failed",
                "Signing in to the community failed. Try again.",
            ),
            CommunityError::SignInTimeout => (
                "community_sign_in_timeout",
                "Sign-in timed out waiting for the browser. Try again.",
            ),
            CommunityError::SignInDenied => (
                "community_sign_in_denied",
                "Sign-in was cancelled on Discord.",
            ),
            CommunityError::Credential(_) => (
                "community_sign_in_failed",
                "The saved community sign-in could not be accessed.",
            ),
            CommunityError::SignedOut => (
                "community_signed_out",
                "Sign in to the community to do this.",
            ),
            CommunityError::Banned => (BANNED_CODE, BANNED_MESSAGE),
            CommunityError::RequestFailed => (
                "community_request_failed",
                "The community request failed. Try again.",
            ),
        };
        CommandError {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<CommunityError> for CommandError {
    fn from(error: CommunityError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::CredentialError;

    fn code(error: CommunityError) -> String {
        CommandError::from(error).code
    }

    #[test]
    fn each_error_has_a_stable_code() {
        assert_eq!(code(CommunityError::Offline), "community_offline");
        assert_eq!(
            code(CommunityError::SignInFailed),
            "community_sign_in_failed"
        );
        assert_eq!(
            code(CommunityError::SignInTimeout),
            "community_sign_in_timeout"
        );
        assert_eq!(
            code(CommunityError::SignInDenied),
            "community_sign_in_denied"
        );
        assert_eq!(
            code(CommunityError::Credential(CredentialError("x".into()))),
            "community_sign_in_failed"
        );
    }

    #[test]
    fn signed_out_and_failed_requests_have_their_own_codes() {
        assert_eq!(code(CommunityError::SignedOut), "community_signed_out");
        assert_eq!(
            code(CommunityError::RequestFailed),
            "community_request_failed"
        );
    }

    #[test]
    fn a_ban_has_its_own_code() {
        assert_eq!(code(CommunityError::Banned), "community_banned");
    }

    #[test]
    fn credential_details_do_not_leak() {
        let error = CommandError::from(CommunityError::Credential(CredentialError(
            "secret-detail".into(),
        )));
        assert!(!error.message.contains("secret-detail"));
    }
}
