use std::fmt;

/// Application-level error. It never wraps `std::io::Error`, `serde_json`,
/// ZIP or Tauri types: adapters translate their failures into these variants,
/// keeping only a textual detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    /// Existing data could not be read.
    Read { location: String, detail: String },
    /// Data could not be written; the previous version is preserved.
    Write { location: String, detail: String },
    /// Existing data is present but invalid (corrupted); it was left untouched.
    Corrupted { location: String, detail: String },
}

impl AppError {
    /// Where the failure happened (usually a file path).
    pub fn location(&self) -> &str {
        match self {
            AppError::Read { location, .. }
            | AppError::Write { location, .. }
            | AppError::Corrupted { location, .. } => location,
        }
    }

    /// Technical detail reported by the adapter.
    pub fn detail(&self) -> &str {
        match self {
            AppError::Read { detail, .. }
            | AppError::Write { detail, .. }
            | AppError::Corrupted { detail, .. } => detail,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self {
            AppError::Read { .. } => "could not read",
            AppError::Write { .. } => "could not write",
            AppError::Corrupted { .. } => "corrupted data in",
        };
        write!(f, "{what} {}: {}", self.location(), self.detail())
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_location_and_detail_for_every_variant() {
        let errors = [
            AppError::Read {
                location: "a".into(),
                detail: "d".into(),
            },
            AppError::Write {
                location: "a".into(),
                detail: "d".into(),
            },
            AppError::Corrupted {
                location: "a".into(),
                detail: "d".into(),
            },
        ];
        for error in &errors {
            assert_eq!(error.location(), "a");
            assert_eq!(error.detail(), "d");
        }
    }

    #[test]
    fn display_mentions_location_and_detail() {
        let error = AppError::Corrupted {
            location: "data/config.json".into(),
            detail: "expected value".into(),
        };
        let text = error.to_string();
        assert!(text.contains("data/config.json"));
        assert!(text.contains("expected value"));
    }
}
