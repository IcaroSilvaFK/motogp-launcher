use crate::AppError;
use motogp_domain::GroupingError;
use std::fmt;

/// Errors of the mod library use cases. Adapters translate their failures
/// (fs, ZIP, JSON) into these variants; no I/O or ZIP type leaks through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// Reading or writing the library storage (index, files, archives) failed.
    Storage(AppError),
    /// A group of files violates a grouping rule (for example, no `.pak`).
    Grouping(GroupingError),
    /// The chosen file extension cannot be imported.
    UnsupportedFormat { extension: String },
    /// The archive contains no `.pak` file.
    NoPakFound,
    /// Two archive entries flatten to the same file name.
    NameCollision { name: String },
    /// The mod ID is not 12 lowercase hexadecimal characters.
    InvalidId { id: String },
    /// No mod with this ID is in the library.
    NotFound { id: String },
}

impl From<AppError> for LibraryError {
    fn from(error: AppError) -> Self {
        LibraryError::Storage(error)
    }
}

impl From<GroupingError> for LibraryError {
    fn from(error: GroupingError) -> Self {
        LibraryError::Grouping(error)
    }
}

impl fmt::Display for LibraryError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for LibraryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_descriptive_english() {
        let cases = [
            (
                LibraryError::UnsupportedFormat {
                    extension: "rar".into(),
                },
                "unsupported format",
            ),
            (LibraryError::NoPakFound, "no .pak"),
            (
                LibraryError::NameCollision {
                    name: "A.pak".into(),
                },
                "A.pak",
            ),
            (LibraryError::InvalidId { id: "../x".into() }, "../x"),
            (LibraryError::NotFound { id: "abc".into() }, "abc"),
            (
                LibraryError::Grouping(GroupingError::MissingPak { group: "A".into() }),
                "has no .pak file",
            ),
            (
                LibraryError::Storage(AppError::Read {
                    location: "library.json".into(),
                    detail: "denied".into(),
                }),
                "library.json",
            ),
        ];
        for (error, expected) in cases {
            assert!(error.to_string().contains(expected), "{error:?} -> {error}");
        }
    }

    #[test]
    fn conversions_wrap_the_source_error() {
        let app = AppError::Write {
            location: "a".into(),
            detail: "b".into(),
        };
        assert_eq!(LibraryError::from(app.clone()), LibraryError::Storage(app));
        let grouping = GroupingError::MissingPak { group: "A".into() };
        assert_eq!(
            LibraryError::from(grouping.clone()),
            LibraryError::Grouping(grouping)
        );
    }
}
