//! Fallback adapter of the mod stars. When the HTTP client cannot be built (see
//! `bootstrap.rs`) the app still starts: the community is simply offline. Also
//! the default of a freshly composed state.
use motogp_application::{CommunityApiError, ModStats, ModStatsApi};
use motogp_domain::SessionToken;

/// A server that can never be reached.
pub struct UnavailableModStatsApi {
    reason: String,
}

impl UnavailableModStatsApi {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn error(&self) -> CommunityApiError {
        CommunityApiError::Unreachable(self.reason.clone())
    }
}

impl ModStatsApi for UnavailableModStatsApi {
    fn stats(&self, _token: Option<&SessionToken>) -> Result<Vec<ModStats>, CommunityApiError> {
        Err(self.error())
    }

    fn star(&self, _token: &SessionToken, _id: &str) -> Result<ModStats, CommunityApiError> {
        Err(self.error())
    }

    fn unstar(&self, _token: &SessionToken, _id: &str) -> Result<ModStats, CommunityApiError> {
        Err(self.error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unavailable_server_is_always_unreachable() {
        let api = UnavailableModStatsApi::new("no tls");
        let token = SessionToken::new("t");
        assert!(matches!(
            api.stats(None),
            Err(CommunityApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.star(&token, "a"),
            Err(CommunityApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.unstar(&token, "a"),
            Err(CommunityApiError::Unreachable(_))
        ));
    }
}
