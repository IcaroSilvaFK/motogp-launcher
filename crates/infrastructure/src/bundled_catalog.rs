//! The catalog snapshot shipped inside the launcher binary (issue #91): the
//! last resort when nothing is cached and the online catalog is unreachable
//! or not published yet.
use crate::parse_catalog;
use motogp_application::{CatalogSourceError, RawCatalog};

/// `catalog/catalog.json` of the repository, embedded at build time.
const BUNDLED_CATALOG_JSON: &str = include_str!("../../../catalog/catalog.json");

/// Parses the embedded catalog into unvalidated drafts.
pub fn bundled_catalog() -> Result<RawCatalog, CatalogSourceError> {
    parse_catalog(BUNDLED_CATALOG_JSON.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::validate_catalog_strict;

    #[test]
    fn the_bundled_catalog_is_the_repository_file_and_passes_the_strict_validation() {
        let file = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalog/catalog.json"),
        )
        .expect("catalog file");

        let bundled = bundled_catalog().expect("bundled catalog parses");

        assert_eq!(bundled, parse_catalog(&file).unwrap());
        assert!(validate_catalog_strict(bundled).is_ok());
    }
}
