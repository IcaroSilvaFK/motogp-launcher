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
        todo!("red")
    }

    /// Technical detail reported by the adapter.
    pub fn detail(&self) -> &str {
        todo!("red")
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!("red")
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
