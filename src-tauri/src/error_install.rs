//! Tauri-boundary mapping for the catalog install errors: stable codes plus a
//! user-facing English message. Catalog and library errors reuse their own
//! mappings; network details are never leaked to the user.
use crate::CommandError;
use motogp_application::CatalogInstallError;

fn error(code: &str, message: impl Into<String>) -> CommandError {
    CommandError {
        code: code.to_string(),
        message: message.into(),
    }
}

impl From<&CatalogInstallError> for CommandError {
    fn from(failure: &CatalogInstallError) -> Self {
        match failure {
            CatalogInstallError::SessionActive => {
                error("library_locked_by_session", failure.to_string())
            }
            CatalogInstallError::EntryNotFound { id } => error(
                "catalog_entry_not_found",
                format!("The catalog has no mod \"{id}\". Refresh the catalog and try again."),
            ),
            CatalogInstallError::NotDownloadable { id } => error(
                "catalog_entry_not_downloadable",
                format!(
                    "The mod \"{id}\" cannot be installed directly. Open its page to download it."
                ),
            ),
            CatalogInstallError::InstallInProgress { id } => error(
                "catalog_install_in_progress",
                format!("The mod \"{id}\" is already being installed."),
            ),
            CatalogInstallError::Network(_) => error(
                "download_network",
                "The download failed. Check your connection and try again.",
            ),
            CatalogInstallError::SizeMismatch { .. } => error(
                "download_size_mismatch",
                "The downloaded file is not the size the catalog announced, so it was discarded.",
            ),
            CatalogInstallError::HashMismatch => error(
                "download_hash_mismatch",
                "The downloaded file does not match the catalog checksum, so it was discarded.",
            ),
            CatalogInstallError::Cancelled => {
                error("download_cancelled", "The install was cancelled.")
            }
            CatalogInstallError::Catalog(inner) => CommandError::from(inner),
            CatalogInstallError::Library(inner) => CommandError::from(inner),
            CatalogInstallError::Storage(_) => error(
                "write_failed",
                "Could not save the download. Check the free space on your disk and try again.",
            ),
        }
    }
}

impl From<CatalogInstallError> for CommandError {
    fn from(failure: CatalogInstallError) -> Self {
        CommandError::from(&failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::{CatalogError, LibraryError};

    fn code(failure: CatalogInstallError) -> String {
        CommandError::from(failure).code
    }

    #[test]
    fn every_install_error_has_a_stable_code() {
        let id = || "rebalanced-ai".to_string();
        assert_eq!(
            code(CatalogInstallError::SessionActive),
            "library_locked_by_session"
        );
        assert_eq!(
            code(CatalogInstallError::EntryNotFound { id: id() }),
            "catalog_entry_not_found"
        );
        assert_eq!(
            code(CatalogInstallError::NotDownloadable { id: id() }),
            "catalog_entry_not_downloadable"
        );
        assert_eq!(
            code(CatalogInstallError::InstallInProgress { id: id() }),
            "catalog_install_in_progress"
        );
        assert_eq!(
            code(CatalogInstallError::Network("dns".into())),
            "download_network"
        );
        assert_eq!(
            code(CatalogInstallError::SizeMismatch {
                expected: 1,
                actual: 2
            }),
            "download_size_mismatch"
        );
        assert_eq!(
            code(CatalogInstallError::HashMismatch),
            "download_hash_mismatch"
        );
        assert_eq!(code(CatalogInstallError::Cancelled), "download_cancelled");
        assert_eq!(
            code(CatalogInstallError::Storage("disk".into())),
            "write_failed"
        );
    }

    #[test]
    fn catalog_and_library_errors_reuse_their_mappings() {
        assert_eq!(
            code(CatalogInstallError::Catalog(CatalogError::Unavailable {
                reason: "x".into()
            })),
            "catalog_unavailable"
        );
        assert_eq!(
            code(CatalogInstallError::Library(LibraryError::NoPakFound)),
            "no_pak_found"
        );
    }

    #[test]
    fn network_details_are_not_leaked() {
        let mapped = CommandError::from(CatalogInstallError::Network("dns failure x.y".into()));
        assert!(!mapped.message.contains("dns"));
    }
}
