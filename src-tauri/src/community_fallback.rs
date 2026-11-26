//! Fallback adapters of the community account. When a production adapter cannot
//! be built (see `bootstrap.rs`) the app still starts: the community is simply
//! offline. Also usable as plain fakes in tests.
use motogp_application::{
    CommunityApi, CommunityApiError, CredentialError, CredentialStore, ServerUser,
};
use motogp_domain::SessionToken;
use std::sync::Mutex;

/// A server that can never be reached.
pub struct UnavailableCommunityApi {
    reason: String,
}

impl UnavailableCommunityApi {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn error(&self) -> CommunityApiError {
        CommunityApiError::Unreachable(self.reason.clone())
    }
}

impl CommunityApi for UnavailableCommunityApi {
    fn start_url(&self, _redirect: &str, _state: &str, _code_challenge: &str) -> String {
        String::new()
    }

    fn exchange(
        &self,
        _code: &str,
        _code_verifier: &str,
    ) -> Result<(SessionToken, ServerUser), CommunityApiError> {
        Err(self.error())
    }

    fn me(&self, _token: &SessionToken) -> Result<ServerUser, CommunityApiError> {
        Err(self.error())
    }

    fn logout(&self, _token: &SessionToken) -> Result<(), CommunityApiError> {
        Err(self.error())
    }
}

/// A token kept in memory only: the session lasts until the launcher closes.
#[derive(Default)]
pub struct MemoryCredentialStore {
    token: Mutex<Option<SessionToken>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
        Ok(self.token.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    fn save(&self, token: &SessionToken) -> Result<(), CredentialError> {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = Some(token.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), CredentialError> {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unavailable_server_is_always_unreachable() {
        let api = UnavailableCommunityApi::new("no tls");
        let token = SessionToken::new("t");
        assert!(matches!(
            api.me(&token),
            Err(CommunityApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.exchange("c", "v"),
            Err(CommunityApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.logout(&token),
            Err(CommunityApiError::Unreachable(_))
        ));
        assert_eq!(api.start_url("r", "s", "c"), "");
    }

    #[test]
    fn the_memory_store_keeps_and_forgets_the_token() {
        let store = MemoryCredentialStore::default();
        assert_eq!(store.load(), Ok(None));
        store.save(&SessionToken::new("t")).unwrap();
        assert_eq!(store.load().unwrap().unwrap().expose(), "t");
        store.delete().unwrap();
        assert_eq!(store.load(), Ok(None));
    }
}
