//! Community account (issue #122): who is signed in to the community server.
use std::fmt;

/// The public profile of the signed-in community member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunityProfile {
    pub display_name: String,
    /// The avatar as a `data:` URL (the WebView CSP only allows `data:` images).
    pub avatar_data_url: Option<String>,
}

/// Where the community sign-in stands right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommunityStatus {
    SignedOut,
    SignedIn(CommunityProfile),
    /// The server could not be reached; a stored session is kept.
    Offline,
}

/// The opaque session token of the community server. It is a secret: `Debug`
/// is redacted and there is no `Serialize`, so it cannot reach logs or IPC by
/// accident. Use [`SessionToken::expose`] only to build the `Authorization`
/// header or to persist it in the credential store.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw secret. Never log or display the result.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_token_debug_output_is_redacted() {
        let token = SessionToken::new("super-secret-token");
        let text = format!("{token:?}");
        assert!(!text.contains("super-secret-token"));
        assert!(text.contains("redacted"));
    }

    #[test]
    fn the_token_debug_stays_redacted_inside_other_values() {
        let wrapped = Some(SessionToken::new("abc123secret"));
        assert!(!format!("{wrapped:?}").contains("abc123secret"));
    }

    #[test]
    fn the_token_exposes_its_value_on_request() {
        assert_eq!(SessionToken::new("t").expose(), "t");
    }

    #[test]
    fn a_status_carries_the_profile() {
        let status = CommunityStatus::SignedIn(CommunityProfile {
            display_name: "rider".into(),
            avatar_data_url: None,
        });
        assert!(matches!(status, CommunityStatus::SignedIn(p) if p.display_name == "rider"));
    }
}
