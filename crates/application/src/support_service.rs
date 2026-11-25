//! Opens the project's support page in the default browser (issue #94).

use crate::WebPageOpener;
use motogp_domain::{check_support_url, SUPPORT_PAGE_URL};
use std::fmt;

/// Why the support page could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportPageError {
    /// The configured URL does not pass the domain policy.
    NotAllowed { reason: String },
    /// The system could not open the page in the browser.
    OpenFailed { reason: String },
}

impl fmt::Display for SupportPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupportPageError::NotAllowed { reason } => {
                write!(f, "support page not allowed: {reason}")
            }
            SupportPageError::OpenFailed { reason } => {
                write!(f, "could not open the support page: {reason}")
            }
        }
    }
}

impl std::error::Error for SupportPageError {}

/// Opens [`SUPPORT_PAGE_URL`] through `opener`, after checking it against the
/// support-page policy.
pub fn open_support_page(opener: &dyn WebPageOpener) -> Result<(), SupportPageError> {
    check_support_url(SUPPORT_PAGE_URL).map_err(|error| SupportPageError::NotAllowed {
        reason: error.to_string(),
    })?;
    opener
        .open_page(SUPPORT_PAGE_URL)
        .map_err(|error| SupportPageError::OpenFailed {
            reason: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppError;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Opener {
        opened: Mutex<Vec<String>>,
        fails: bool,
    }

    impl WebPageOpener for Opener {
        fn open_page(&self, url: &str) -> Result<(), AppError> {
            if self.fails {
                return Err(AppError::Read {
                    location: url.to_string(),
                    detail: "no browser".into(),
                });
            }
            self.opened.lock().unwrap().push(url.to_string());
            Ok(())
        }
    }

    #[test]
    fn opens_exactly_the_configured_url() {
        let opener = Opener::default();
        assert_eq!(open_support_page(&opener), Ok(()));
        assert_eq!(*opener.opened.lock().unwrap(), vec![SUPPORT_PAGE_URL]);
    }

    #[test]
    fn opener_failure_is_reported() {
        let opener = Opener {
            fails: true,
            ..Opener::default()
        };
        assert!(matches!(
            open_support_page(&opener),
            Err(SupportPageError::OpenFailed { .. })
        ));
    }
}
