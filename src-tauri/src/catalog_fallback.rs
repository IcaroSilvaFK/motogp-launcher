//! Catalog source used when the HTTPS client cannot be built: the catalog is
//! then served from the offline cache only, and the app still starts.
use motogp_application::{CatalogSource, CatalogSourceError, SourceFetch};

/// A source that always fails as a network error.
pub struct UnavailableCatalogSource {
    reason: String,
}

impl UnavailableCatalogSource {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl CatalogSource for UnavailableCatalogSource {
    fn fetch(&self, _etag: Option<&str>) -> Result<SourceFetch, CatalogSourceError> {
        Err(CatalogSourceError::Network(self.reason.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_always_fails_as_a_network_error() {
        let source = UnavailableCatalogSource::new("no tls");
        assert_eq!(
            source.fetch(Some("etag")),
            Err(CatalogSourceError::Network("no tls".to_string()))
        );
    }
}
