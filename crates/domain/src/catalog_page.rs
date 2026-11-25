//! Policy for opening a catalog entry's web page in the default browser
//! (issue #73). Pure, no I/O.
//!
//! Only `https` pages on a small allow-list of hosts may be opened, so a
//! tampered catalog cannot make the launcher open an arbitrary site or a
//! non-web scheme.

use std::fmt;

/// Hosts a catalog page may live on. A host matches exactly or as a
/// subdomain (`www.overtake.gg` matches `overtake.gg`; `evilovertake.gg` does not).
pub const ALLOWED_PAGE_HOSTS: [&str; 3] = ["overtake.gg", "github.com", "nexusmods.com"];

/// Why a page URL may not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageUrlError {
    /// The URL is not a well-formed `https://` URL (other scheme, no host,
    /// whitespace, or credentials in it).
    NotHttps,
    /// The host is not on the allow-list.
    HostNotAllowed { host: String },
}

impl fmt::Display for PageUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PageUrlError::NotHttps => write!(f, "page URL must be a plain https URL"),
            PageUrlError::HostNotAllowed { host } => {
                write!(f, "page host {host} is not allowed")
            }
        }
    }
}

impl std::error::Error for PageUrlError {}

/// Checks that `url` is an `https` URL without credentials whose host is on
/// the allow-list (exact match or dot-suffix, case-insensitive).
pub fn check_page_url(url: &str) -> Result<(), PageUrlError> {
    let scheme_ok = url
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
    if !scheme_ok || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(PageUrlError::NotHttps);
    }
    let authority = url[8..].split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains(['@', '\\']) {
        return Err(PageUrlError::NotHttps);
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if port.is_some_and(|p| p != "443") {
        return Err(PageUrlError::NotHttps);
    }
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let allowed = ALLOWED_PAGE_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")));
    if allowed {
        Ok(())
    } else {
        Err(PageUrlError::HostNotAllowed { host })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_hosts_and_subdomains_pass() {
        for url in [
            "https://overtake.gg/downloads/some-mod.1/",
            "https://www.overtake.gg/x",
            "HTTPS://OverTake.GG/x",
            "https://github.com/user/repo",
            "https://nexusmods.com/motogp26/mods/1",
            "https://www.nexusmods.com/motogp26/mods/1?a=b#c",
            "https://overtake.gg:443/x",
            "https://overtake.gg",
        ] {
            assert_eq!(check_page_url(url), Ok(()), "{url}");
        }
    }

    #[test]
    fn non_https_or_malformed_urls_are_rejected() {
        for url in [
            "http://overtake.gg/x",
            "ftp://overtake.gg/x",
            "steam://run/1",
            "javascript:alert(1)",
            "https://",
            "https:///x",
            "",
            "https://user@overtake.gg/x",
            "https://user:pw@overtake.gg/x",
            "https://overtake.gg\\@evil.com/",
            "https://overtake.gg/a b",
            "https://overtake.gg:8443/x",
            "https://overtake.gg:evil/x",
        ] {
            assert_eq!(check_page_url(url), Err(PageUrlError::NotHttps), "{url}");
        }
    }

    #[test]
    fn hosts_outside_the_allow_list_are_rejected() {
        for url in [
            "https://evilovertake.gg/x",
            "https://overtake.gg.evil.com/x",
            "https://example.com/x",
            "https://notgithub.com/x",
            "https://github.com.evil.io/x",
        ] {
            assert!(
                matches!(
                    check_page_url(url),
                    Err(PageUrlError::HostNotAllowed { .. })
                ),
                "{url}"
            );
        }
    }

    #[test]
    fn errors_have_readable_messages() {
        assert!(PageUrlError::NotHttps.to_string().contains("https"));
        let error = PageUrlError::HostNotAllowed {
            host: "a.com".into(),
        };
        assert!(error.to_string().contains("a.com"));
        let _: &dyn std::error::Error = &error;
    }
}
