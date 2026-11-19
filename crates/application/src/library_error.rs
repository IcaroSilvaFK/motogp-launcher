use crate::AppError;
use motogp_domain::GroupingError;
use std::fmt;

/// Which ZIP extraction limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveLimit {
    /// More entries than allowed.
    EntryCount,
    /// One entry extracts to more bytes than allowed.
    EntryBytes,
    /// All entries together extract to more bytes than allowed.
    TotalBytes,
}

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
    /// The archive exceeds an extraction limit (zip bomb protection).
    ArchiveLimitExceeded { limit: ArchiveLimit },
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Storage(error) => write!(f, "{error}"),
            LibraryError::Grouping(error) => write!(f, "{error}"),
            LibraryError::UnsupportedFormat { extension } if extension.is_empty() => write!(
                f,
                "unsupported format: the file has no extension (expected .pak, .utoc, .ucas or .zip)"
            ),
            LibraryError::UnsupportedFormat { extension } => write!(
                f,
                "unsupported format: .{extension} (expected .pak, .utoc, .ucas or .zip)"
            ),
            LibraryError::NoPakFound => write!(f, "no .pak file found in the archive"),
            LibraryError::NameCollision { name } => write!(
                f,
                "two files in the archive flatten to the same name: {name}"
            ),
            LibraryError::InvalidId { id } => write!(f, "invalid mod ID: {id:?}"),
            LibraryError::NotFound { id } => write!(f, "no mod with ID {id} in the library"),
            LibraryError::ArchiveLimitExceeded { limit } => write!(
                f,
                "the archive is too large to import: {}",
                match limit {
                    ArchiveLimit::EntryCount => "too many entries",
                    ArchiveLimit::EntryBytes => "an entry is too big",
                    ArchiveLimit::TotalBytes => "the extracted size is too big",
                }
            ),
        }
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
            (
                LibraryError::UnsupportedFormat {
                    extension: String::new(),
                },
                "no extension",
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
