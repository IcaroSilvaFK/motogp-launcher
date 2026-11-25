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
            CatalogError::UnsupportedSchema { .. } => (
                "catalog_unsupported_schema",
                "The mod catalog needs a newer version of the launcher. Update the launcher to browse it.",
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
    fn unsupported_schema_maps_to_a_stable_code() {
        let error = CommandError::from(CatalogError::UnsupportedSchema { version: 2 });
        assert_eq!(error.code, "catalog_unsupported_schema");
        assert!(error.message.contains("newer version"));
    }
}
