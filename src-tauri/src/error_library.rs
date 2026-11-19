//! Error mapping at the Tauri boundary for the mod library use cases.
//! Storage failures reuse the shared `AppError` mapping; the library-specific
//! variants get their own stable code and message.
use crate::CommandError;
use motogp_application::LibraryError;
use motogp_domain::GroupingError;

/// Stable code and user-facing text (currently English) for the
/// library-specific variants; `Storage` is delegated to the shared mapping.
impl From<&LibraryError> for CommandError {
    fn from(error: &LibraryError) -> Self {
        let (code, message) = match error {
            LibraryError::Storage(app_error) => return CommandError::from(app_error),
            LibraryError::Grouping(GroupingError::MissingPak { group }) => (
                "mod_without_pak",
                format!("The mod \"{group}\" has no .pak file."),
            ),
            LibraryError::UnsupportedFormat { extension } => (
                "unsupported_format",
                format!(
                "Unsupported format \".{extension}\". Import a .pak, .utoc, .ucas or .zip file."
            ),
            ),
            LibraryError::NoPakFound => (
                "no_pak_found",
                "The archive does not contain any .pak file.".to_string(),
            ),
            LibraryError::NameCollision { name } => (
                "name_collision",
                format!("The archive has more than one file named {name} in different folders."),
            ),
            LibraryError::InvalidId { id } => {
                ("invalid_mod_id", format!("\"{id}\" is not a valid mod ID."))
            }
            LibraryError::NotFound { id } => (
                "mod_not_found",
                format!("No mod with ID {id} exists in the library."),
            ),
            LibraryError::ArchiveLimitExceeded { .. } => (
                "archive_too_large",
                "The archive is too large to import (possible zip bomb).".to_string(),
            ),
        };
        CommandError {
            code: code.to_string(),
            message,
        }
    }
}

impl From<LibraryError> for CommandError {
    fn from(error: LibraryError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;
    use motogp_domain::GroupingError;

    fn code_of(error: LibraryError) -> String {
        CommandError::from(error).code
    }

    #[test]
    fn maps_each_variant_to_a_stable_code() {
        assert_eq!(
            code_of(LibraryError::UnsupportedFormat {
                extension: "rar".into()
            }),
            "unsupported_format"
        );
        assert_eq!(code_of(LibraryError::NoPakFound), "no_pak_found");
        assert_eq!(
            code_of(LibraryError::ArchiveLimitExceeded {
                limit: motogp_application::ArchiveLimit::TotalBytes
            }),
            "archive_too_large"
        );
        assert_eq!(
            code_of(LibraryError::NameCollision {
                name: "A.pak".into()
            }),
            "name_collision"
        );
        assert_eq!(
            code_of(LibraryError::InvalidId { id: "x".into() }),
            "invalid_mod_id"
        );
        assert_eq!(
            code_of(LibraryError::NotFound { id: "x".into() }),
            "mod_not_found"
        );
        assert_eq!(
            code_of(LibraryError::Grouping(GroupingError::MissingPak {
                group: "A".into()
            })),
            "mod_without_pak"
        );
    }

    #[test]
    fn storage_errors_reuse_the_app_error_mapping() {
        let app = AppError::Corrupted {
            location: "library.json".into(),
            detail: "bad".into(),
        };
        assert_eq!(
            CommandError::from(LibraryError::Storage(app.clone())),
            CommandError::from(app)
        );
    }

    #[test]
    fn messages_are_user_facing_and_mention_the_subject() {
        let cases = [
            (
                LibraryError::UnsupportedFormat {
                    extension: "rar".into(),
                },
                "rar",
            ),
            (LibraryError::NoPakFound, ".pak"),
            (
                LibraryError::NameCollision {
                    name: "A.pak".into(),
                },
                "A.pak",
            ),
            (LibraryError::InvalidId { id: "../x".into() }, "../x"),
            (
                LibraryError::NotFound {
                    id: "abc123".into(),
                },
                "abc123",
            ),
            (
                LibraryError::Grouping(GroupingError::MissingPak { group: "Z".into() }),
                "Z",
            ),
        ];
        for (error, expected) in cases {
            let mapped = CommandError::from(&error);
            assert!(
                mapped.message.contains(expected),
                "{error:?}: {}",
                mapped.message
            );
        }
    }

    #[test]
    fn serializes_as_code_and_message() {
        let value = serde_json::to_value(CommandError::from(LibraryError::NoPakFound)).unwrap();
        assert_eq!(value["code"], "no_pak_found");
        assert!(value["message"].is_string());
    }
}
