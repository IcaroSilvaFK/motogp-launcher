//! Tauri-boundary mapping for the support-page use-case errors: one stable
//! code and a fixed English message, no internal details.
use crate::CommandError;
use motogp_application::SupportPageError;

impl From<SupportPageError> for CommandError {
    fn from(_error: SupportPageError) -> Self {
        CommandError {
            code: "open_page_failed".to_string(),
            message: "The page could not be opened in your browser.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_support_error_maps_to_open_page_failed_without_details() {
        for error in [
            SupportPageError::NotAllowed {
                reason: "host evil.com".into(),
            },
            SupportPageError::OpenFailed {
                reason: "os detail".into(),
            },
        ] {
            let mapped = CommandError::from(error);
            assert_eq!(mapped.code, "open_page_failed");
            assert_eq!(
                mapped.message,
                "The page could not be opened in your browser."
            );
        }
    }
}
