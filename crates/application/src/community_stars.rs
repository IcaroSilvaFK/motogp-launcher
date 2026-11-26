//! Use case: mod stars (issue #119). The service loads the session token from
//! the credential store itself, so the token never reaches the interface layer.
use crate::{CommunityApiError, CommunityError, CredentialStore};
use motogp_domain::SessionToken;
use std::sync::Arc;

/// Community numbers of one mod, as `GET /mods/stats` returns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStats {
    /// Catalog slug of the mod.
    pub id: String,
    pub stars: u64,
    /// Present only when the request carried a valid session.
    pub starred_by_me: Option<bool>,
}

/// Port: the community server's stars endpoints.
pub trait ModStatsApi: Send + Sync {
    /// Stats of every mod with stars. With a token, `starred_by_me` is filled;
    /// a stale token is answered anonymously by the server.
    fn stats(&self, token: Option<&SessionToken>) -> Result<Vec<ModStats>, CommunityApiError>;
    /// Idempotent: starring twice keeps one star.
    fn star(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError>;
    /// Idempotent: unstarring a mod that is not starred is a success.
    fn unstar(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError>;
}

pub struct CommunityStarsService {
    api: Arc<dyn ModStatsApi>,
    store: Arc<dyn CredentialStore>,
}

impl CommunityStarsService {
    pub fn new(api: Arc<dyn ModStatsApi>, store: Arc<dyn CredentialStore>) -> Self {
        Self { api, store }
    }

    /// Stats of the mods with stars. Sends the session when there is one; an
    /// unreadable credential store just means an anonymous request.
    pub fn stats(&self) -> Result<Vec<ModStats>, CommunityError> {
        let token = self.store.load().ok().flatten();
        self.api.stats(token.as_ref()).map_err(map_error)
    }

    pub fn star(&self, id: &str) -> Result<ModStats, CommunityError> {
        let token = self.token()?;
        self.api.star(&token, id).map_err(map_error)
    }

    pub fn unstar(&self, id: &str) -> Result<ModStats, CommunityError> {
        let token = self.token()?;
        self.api.unstar(&token, id).map_err(map_error)
    }

    fn token(&self) -> Result<SessionToken, CommunityError> {
        self.store.load()?.ok_or(CommunityError::SignedOut)
    }
}

fn map_error(error: CommunityApiError) -> CommunityError {
    match error {
        CommunityApiError::Unauthorized => CommunityError::SignedOut,
        CommunityApiError::Unreachable(_) => CommunityError::Offline,
        CommunityApiError::Protocol(_) => CommunityError::RequestFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CredentialError;
    use std::sync::Mutex;

    const TOKEN: &str = "tok-SECRET";

    fn stats(id: &str, stars: u64, mine: Option<bool>) -> ModStats {
        ModStats {
            id: id.into(),
            stars,
            starred_by_me: mine,
        }
    }

    /// Records the token each call carried and answers with a canned result.
    struct FakeApi {
        answer: Mutex<Result<Vec<ModStats>, CommunityApiError>>,
        calls: Mutex<Vec<(String, Option<String>)>>,
    }

    impl FakeApi {
        fn new(answer: Result<Vec<ModStats>, CommunityApiError>) -> Arc<Self> {
            Arc::new(Self {
                answer: Mutex::new(answer),
                calls: Mutex::default(),
            })
        }
        fn record(&self, what: String, token: Option<&SessionToken>) {
            self.calls
                .lock()
                .unwrap()
                .push((what, token.map(|t| t.expose().to_string())));
        }
        fn one(&self) -> Result<ModStats, CommunityApiError> {
            self.answer
                .lock()
                .unwrap()
                .clone()
                .map(|mut list| list.remove(0))
        }
    }

    impl ModStatsApi for FakeApi {
        fn stats(&self, token: Option<&SessionToken>) -> Result<Vec<ModStats>, CommunityApiError> {
            self.record("stats".into(), token);
            self.answer.lock().unwrap().clone()
        }
        fn star(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
            self.record(format!("star {id}"), Some(token));
            self.one()
        }
        fn unstar(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
            self.record(format!("unstar {id}"), Some(token));
            self.one()
        }
    }

    struct FakeStore(Result<Option<SessionToken>, CredentialError>);

    impl CredentialStore for FakeStore {
        fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
            self.0.clone()
        }
        fn save(&self, _token: &SessionToken) -> Result<(), CredentialError> {
            Ok(())
        }
        fn delete(&self) -> Result<(), CredentialError> {
            Ok(())
        }
    }

    fn service(
        api: &Arc<FakeApi>,
        token: Result<Option<SessionToken>, CredentialError>,
    ) -> CommunityStarsService {
        CommunityStarsService::new(api.clone(), Arc::new(FakeStore(token)))
    }

    fn signed_in() -> Result<Option<SessionToken>, CredentialError> {
        Ok(Some(SessionToken::new(TOKEN)))
    }

    #[test]
    fn stats_send_the_session_when_there_is_one() {
        let api = FakeApi::new(Ok(vec![stats("a", 3, Some(true))]));
        let list = service(&api, signed_in()).stats().unwrap();
        assert_eq!(list, vec![stats("a", 3, Some(true))]);
        assert_eq!(
            api.calls.lock().unwrap()[0],
            ("stats".to_string(), Some(TOKEN.to_string()))
        );
    }

    #[test]
    fn stats_are_anonymous_when_signed_out_or_the_store_fails() {
        for token in [Ok(None), Err(CredentialError("locked".into()))] {
            let api = FakeApi::new(Ok(vec![stats("a", 3, None)]));
            assert_eq!(service(&api, token).stats().unwrap().len(), 1);
            assert_eq!(api.calls.lock().unwrap()[0].1, None);
        }
    }

    #[test]
    fn stats_of_an_unreachable_server_are_offline() {
        let api = FakeApi::new(Err(CommunityApiError::Unreachable("dns".into())));
        assert_eq!(
            service(&api, signed_in()).stats(),
            Err(CommunityError::Offline)
        );
    }

    #[test]
    fn star_and_unstar_send_the_stored_token() {
        let api = FakeApi::new(Ok(vec![stats("a", 11, Some(true))]));
        let svc = service(&api, signed_in());
        assert_eq!(svc.star("a").unwrap(), stats("a", 11, Some(true)));
        assert_eq!(svc.unstar("a").unwrap(), stats("a", 11, Some(true)));
        let calls = api.calls.lock().unwrap();
        assert_eq!(calls[0], ("star a".to_string(), Some(TOKEN.to_string())));
        assert_eq!(calls[1], ("unstar a".to_string(), Some(TOKEN.to_string())));
    }

    #[test]
    fn starring_without_a_session_is_signed_out_and_never_calls_the_server() {
        let api = FakeApi::new(Ok(vec![stats("a", 1, None)]));
        let svc = service(&api, Ok(None));
        assert_eq!(svc.star("a"), Err(CommunityError::SignedOut));
        assert_eq!(svc.unstar("a"), Err(CommunityError::SignedOut));
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_credential_failure_is_reported_when_starring() {
        let api = FakeApi::new(Ok(vec![stats("a", 1, None)]));
        let svc = service(&api, Err(CredentialError("locked".into())));
        assert!(matches!(svc.star("a"), Err(CommunityError::Credential(_))));
    }

    #[test]
    fn server_errors_map_to_community_errors() {
        let cases = [
            (CommunityApiError::Unauthorized, CommunityError::SignedOut),
            (
                CommunityApiError::Unreachable("x".into()),
                CommunityError::Offline,
            ),
            (
                CommunityApiError::Protocol("HTTP 429".into()),
                CommunityError::RequestFailed,
            ),
        ];
        for (api_error, expected) in cases {
            let api = FakeApi::new(Err(api_error));
            let svc = service(&api, signed_in());
            assert_eq!(svc.star("a"), Err(expected.clone()));
            assert_eq!(svc.unstar("a"), Err(expected));
        }
    }
}
