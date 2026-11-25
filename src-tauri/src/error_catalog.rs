//! Tauri-boundary mapping for the catalog use-case errors: stable codes plus
//! a user-facing English message.
use crate::CommandError;
use motogp_application::CatalogError;

impl From<&CatalogError> for CommandError {
    fn from(error: &CatalogError) -> Self {
        let (code, message) = match error {
            CatalogError::Unavailable { .. } => (
                "catalog_unavailable",
                "The mod catalog could not be loaded. Check your connection and try again.",
            ),
            CatalogError::NotPublished => (
                "catalog_not_published",
                "No mod catalog has been published yet. Try again later.",
            ),
            CatalogError::UnsupportedSchema { .. } => (
                "catalog_unsupported_schema",
                "The mod catalog needs a newer version of the launcher. Update the launcher to browse it.",
            ),
            CatalogError::EntryNotFound { .. } => (
                "catalog_entry_not_found",
                "That mod is no longer in the catalog. Refresh the catalog and try again.",
            ),
            CatalogError::PageNotAllowed { .. } => (
                "catalog_page_not_allowed",
                "The page of this mod cannot be opened because its address is not trusted.",
            ),
            CatalogError::OpenFailed { .. } => (
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

impl From<CatalogError> for CommandError {
    fn from(error: CatalogError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_maps_to_a_stable_code_without_leaking_the_reason() {
        let error = CommandError::from(CatalogError::Unavailable {
            reason: "network error: dns".to_string(),
        });
        assert_eq!(error.code, "catalog_unavailable");
        assert!(!error.message.contains("dns"));
    }

    #[test]
    fn not_published_has_its_own_stable_code() {
        let error = CommandError::from(CatalogError::NotPublished);
        assert_eq!(error.code, "catalog_not_published");
        assert!(error.message.contains("No mod catalog has been published"));
    }

    #[test]
    fn unsupported_schema_maps_to_a_stable_code() {
        let error = CommandError::from(CatalogError::UnsupportedSchema { version: 2 });
        assert_eq!(error.code, "catalog_unsupported_schema");
        assert!(error.message.contains("newer version"));
    }

    #[test]
    fn open_page_errors_map_to_stable_codes_without_leaking_details() {
        let cases = [
            (
                CatalogError::EntryNotFound {
                    id: "secret-id".into(),
                },
                "catalog_entry_not_found",
            ),
            (
                CatalogError::PageNotAllowed {
                    id: "secret-id".into(),
                    reason: "host evil.com".into(),
                },
                "catalog_page_not_allowed",
            ),
            (
                CatalogError::OpenFailed {
                    reason: "os detail".into(),
                },
                "open_page_failed",
            ),
        ];
        for (error, code) in cases {
            let mapped = CommandError::from(error);
            assert_eq!(mapped.code, code);
            assert!(!mapped.message.is_empty());
            assert!(!mapped.message.contains("secret-id"));
            assert!(!mapped.message.contains("evil.com"));
            assert!(!mapped.message.contains("os detail"));
        }
    }
}
