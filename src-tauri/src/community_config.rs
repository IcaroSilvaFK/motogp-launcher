//! Where the community server lives (issue #122). The single place that knows
//! the URL: change it here when the real server exists.

/// Debug builds talk to a local server (`server/`, default bind address).
#[cfg(debug_assertions)]
pub const COMMUNITY_SERVER_URL: &str = "http://127.0.0.1:8787";

/// PLACEHOLDER until the VPS exists: the `.invalid` TLD never resolves, so a
/// release build reports the community as offline instead of calling a stranger.
#[cfg(not(debug_assertions))]
pub const COMMUNITY_SERVER_URL: &str = "https://community.PLACEHOLDER.invalid";

/// Path (with the query marker) of the sign-in start page the browser may open.
pub(crate) const START_PATH: &str = "/auth/discord/start?";

/// True when `url` is the community sign-in start page on the configured server.
pub(crate) fn is_community_start_url(url: &str) -> bool {
    url.strip_prefix(COMMUNITY_SERVER_URL)
        .is_some_and(|rest| rest.starts_with(START_PATH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_start_page_of_the_configured_server_is_allowed() {
        let ok = format!("{COMMUNITY_SERVER_URL}/auth/discord/start?redirect=x&state=y");
        assert!(is_community_start_url(&ok));
        for bad in [
            "https://evil.example/auth/discord/start?x=1".to_string(),
            format!("{COMMUNITY_SERVER_URL}/other?x=1"),
            format!("{COMMUNITY_SERVER_URL}.evil.example/auth/discord/start?x=1"),
            format!("{COMMUNITY_SERVER_URL}/auth/discord/start"),
            "javascript:alert(1)".to_string(),
        ] {
            assert!(!is_community_start_url(&bad), "{bad}");
        }
    }

    #[test]
    fn debug_builds_use_the_local_server() {
        #[cfg(debug_assertions)]
        assert_eq!(COMMUNITY_SERVER_URL, "http://127.0.0.1:8787");
    }
}
