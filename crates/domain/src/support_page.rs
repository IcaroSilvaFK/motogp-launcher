//! The project's support page (issue #94). Pure, no I/O.
//!
//! The URL is a compile-time constant, never user input. It is still checked
//! against its own single-host allow-list, separate from the catalog one, so
//! a bad edit cannot make the launcher open an arbitrary site.

use crate::catalog_page::{check_https_url_on_hosts, PageUrlError};

// PLACEHOLDER: replace with the Apoia.se campaign URL once the user provides it (issue #94).
/// The support page opened in the default browser.
pub const SUPPORT_PAGE_URL: &str = "https://apoia.se/";

/// The only host (or a subdomain of it) the support page may live on.
pub const SUPPORT_PAGE_HOST: &str = "apoia.se";

/// Checks that `url` is a plain `https` URL on [`SUPPORT_PAGE_HOST`].
pub fn check_support_url(url: &str) -> Result<(), PageUrlError> {
    check_https_url_on_hosts(url, &[SUPPORT_PAGE_HOST])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check_page_url;

    #[test]
    fn the_configured_url_passes() {
        assert_eq!(check_support_url(SUPPORT_PAGE_URL), Ok(()));
    }

    #[test]
    fn subdomain_and_path_pass() {
        assert_eq!(check_support_url("https://www.apoia.se/campaign"), Ok(()));
    }

    #[test]
    fn lookalike_hosts_and_other_schemes_fail() {
        for url in [
            "https://evilapoia.se/",
            "https://apoia.se.evil.com/",
            "http://apoia.se/",
            "https://user@apoia.se/",
        ] {
            assert!(check_support_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn catalog_policy_does_not_allow_the_support_host() {
        assert_eq!(
            check_page_url("https://apoia.se/x"),
            Err(PageUrlError::HostNotAllowed {
                host: "apoia.se".into()
            })
        );
    }
}
