//! Tauri-boundary mapping for the ad use-case errors: stable codes plus a
//! user-facing English message. Details (slot, id, OS reason) never leak.
use crate::CommandError;
use motogp_application::AdError;

/// The error for a slot or creative id the launcher does not offer. Unknown
/// slot names get the same answer as unknown ids.
pub(crate) fn unknown_creative() -> CommandError {
    CommandError {
        code: "ad_unknown_creative".to_string(),
        message: "This sponsor is no longer available.".to_string(),
    }
}

impl From<&AdError> for CommandError {
    fn from(error: &AdError) -> Self {
        let (code, message) = match error {
            AdError::UnknownCreative { .. } => (
                "ad_unknown_creative",
                "This sponsor is no longer available.",
            ),
            AdError::Open(_) => (
                "open_page_failed",
                "The page could not be opened in your browser.",
            ),
        };
        CommandError {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

impl From<AdError> for CommandError {
    fn from(error: AdError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;
    use motogp_domain::AdSlot;

    #[test]
    fn an_unknown_creative_maps_to_a_stable_code_without_leaking_the_id() {
        let error = CommandError::from(AdError::UnknownCreative {
            slot: AdSlot::Discover,
            id: "secret-id".into(),
        });
        assert_eq!(error.code, "ad_unknown_creative");
        assert_eq!(error.message, "This sponsor is no longer available.");
        assert!(!error.message.contains("secret-id"));
    }

    #[test]
    fn an_unknown_slot_uses_the_same_error_as_an_unknown_id() {
        assert_eq!(unknown_creative().code, "ad_unknown_creative");
    }

    #[test]
    fn a_failed_open_reuses_the_open_page_code_without_leaking_details() {
        let error = CommandError::from(AdError::Open(AppError::Read {
            location: "https://sponsor.example/secret".into(),
            detail: "os detail".into(),
        }));
        assert_eq!(error.code, "open_page_failed");
        assert!(!error.message.contains("sponsor.example"));
        assert!(!error.message.contains("os detail"));
    }
}
