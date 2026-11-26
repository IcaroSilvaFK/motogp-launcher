//! Use case: the community account (issue #122). Sign in with Discord through
//! the system browser and a loopback callback, keep the session token in the
//! credential store and never expose it.
use crate::{
    AvatarFetcher, CallbackParams, CommunityApi, CommunityApiError, CommunityError,
    CredentialStore, LoopbackError, LoopbackReceiver, PkceSource, ServerUser, WebPageOpener,
};
use motogp_domain::{CommunityProfile, CommunityStatus};
use std::sync::Arc;
use std::time::Duration;

/// How long sign-in waits for the browser to come back.
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(120);

pub struct CommunityAuthService {
    api: Arc<dyn CommunityApi>,
    store: Arc<dyn CredentialStore>,
    loopback: Arc<dyn LoopbackReceiver>,
    pkce: Arc<dyn PkceSource>,
    opener: Arc<dyn WebPageOpener>,
    avatars: Arc<dyn AvatarFetcher>,
    timeout: Duration,
}

impl CommunityAuthService {
    pub fn new(
        api: Arc<dyn CommunityApi>,
        store: Arc<dyn CredentialStore>,
        loopback: Arc<dyn LoopbackReceiver>,
        pkce: Arc<dyn PkceSource>,
        opener: Arc<dyn WebPageOpener>,
        avatars: Arc<dyn AvatarFetcher>,
    ) -> Self {
        Self {
            api,
            store,
            loopback,
            pkce,
            opener,
            avatars,
            timeout: SIGN_IN_TIMEOUT,
        }
    }

    /// Overrides the browser wait (tests).
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    fn profile(&self, user: ServerUser) -> CommunityProfile {
        let avatar_data_url = user.avatar_url.and_then(|url| self.avatars.fetch(&url));
        CommunityProfile {
            display_name: user.display_name,
            avatar_data_url,
        }
    }

    /// The current state. A rejected session is dropped quietly (signed out);
    /// an unreachable server keeps the stored session and reports offline.
    pub fn status(&self) -> Result<CommunityStatus, CommunityError> {
        let Some(token) = self.store.load()? else {
            return Ok(CommunityStatus::SignedOut);
        };
        match self.api.me(&token) {
            Ok(user) => Ok(CommunityStatus::SignedIn(self.profile(user))),
            Err(CommunityApiError::Unauthorized) => {
                self.store.delete()?;
                Ok(CommunityStatus::SignedOut)
            }
            Err(CommunityApiError::Unreachable(_) | CommunityApiError::Protocol(_)) => {
                Ok(CommunityStatus::Offline)
            }
        }
    }

    /// Runs the whole browser flow. The token is stored only after the server
    /// accepted the one-time code.
    pub fn sign_in(&self) -> Result<CommunityStatus, CommunityError> {
        let session = self
            .loopback
            .bind()
            .map_err(|_| CommunityError::SignInFailed)?;
        let state = self.pkce.state();
        let verifier = self.pkce.verifier();
        let challenge = self.pkce.challenge(&verifier);
        let url = self
            .api
            .start_url(&session.redirect_url(), &state, &challenge);
        self.opener
            .open_page(&url)
            .map_err(|_| CommunityError::SignInFailed)?;
        let params = session.wait(self.timeout).map_err(|error| match error {
            LoopbackError::Timeout => CommunityError::SignInTimeout,
            LoopbackError::Io(_) => CommunityError::SignInFailed,
        })?;
        let code = check_callback(params, &state)?;
        let (token, user) = self
            .api
            .exchange(&code, &verifier)
            .map_err(|error| match error {
                CommunityApiError::Unreachable(_) => CommunityError::Offline,
                _ => CommunityError::SignInFailed,
            })?;
        self.store.save(&token)?;
        Ok(CommunityStatus::SignedIn(self.profile(user)))
    }

    /// Revokes the session on the server when it can, and always forgets it
    /// locally, so being offline never traps the user in a signed-in state.
    pub fn sign_out(&self) -> Result<(), CommunityError> {
        if let Some(token) = self.store.load()? {
            // Best effort: an expired session or an offline server is fine.
            let _ = self.api.logout(&token);
        }
        self.store.delete()?;
        Ok(())
    }
}

/// Validates the browser callback and returns the one-time code.
fn check_callback(params: CallbackParams, state: &str) -> Result<String, CommunityError> {
    if params.state.as_deref() != Some(state) {
        return Err(CommunityError::SignInFailed);
    }
    if let Some(error) = params.error {
        return Err(if error == "access_denied" {
            CommunityError::SignInDenied
        } else {
            CommunityError::SignInFailed
        });
    }
    params.code.ok_or(CommunityError::SignInFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppError, CredentialError, LoopbackSession};
    use motogp_domain::SessionToken;
    use std::sync::Mutex;

    const TOKEN: &str = "tok-SECRET-123";

    #[derive(Default)]
    struct FakeApi {
        me: Mutex<Option<Result<ServerUser, CommunityApiError>>>,
        exchange: Mutex<Option<Result<(SessionToken, ServerUser), CommunityApiError>>>,
        logout: Mutex<Option<Result<(), CommunityApiError>>>,
        logouts: Mutex<Vec<String>>,
        exchanged: Mutex<Vec<(String, String)>>,
        starts: Mutex<Vec<(String, String, String)>>,
    }

    fn rider() -> ServerUser {
        ServerUser {
            display_name: "rider".into(),
            avatar_url: Some("https://cdn.example/a.png".into()),
        }
    }

    impl CommunityApi for FakeApi {
        fn start_url(&self, redirect: &str, state: &str, challenge: &str) -> String {
            self.starts
                .lock()
                .unwrap()
                .push((redirect.into(), state.into(), challenge.into()));
            format!("https://server.example/start?state={state}")
        }
        fn exchange(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<(SessionToken, ServerUser), CommunityApiError> {
            self.exchanged
                .lock()
                .unwrap()
                .push((code.into(), verifier.into()));
            self.exchange
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok((SessionToken::new(TOKEN), rider())))
        }
        fn me(&self, _token: &SessionToken) -> Result<ServerUser, CommunityApiError> {
            self.me
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok(rider()))
        }
        fn logout(&self, token: &SessionToken) -> Result<(), CommunityApiError> {
            self.logouts.lock().unwrap().push(token.expose().into());
            self.logout.lock().unwrap().take().unwrap_or(Ok(()))
        }
    }

    #[derive(Default)]
    struct FakeStore {
        token: Mutex<Option<SessionToken>>,
        fail_load: bool,
    }

    impl FakeStore {
        fn holding(token: &str) -> Self {
            Self {
                token: Mutex::new(Some(SessionToken::new(token))),
                fail_load: false,
            }
        }
        fn has_token(&self) -> bool {
            self.token.lock().unwrap().is_some()
        }
    }

    impl CredentialStore for FakeStore {
        fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
            if self.fail_load {
                return Err(CredentialError("locked".into()));
            }
            Ok(self.token.lock().unwrap().clone())
        }
        fn save(&self, token: &SessionToken) -> Result<(), CredentialError> {
            *self.token.lock().unwrap() = Some(token.clone());
            Ok(())
        }
        fn delete(&self) -> Result<(), CredentialError> {
            *self.token.lock().unwrap() = None;
            Ok(())
        }
    }

    struct FakeSession {
        outcome: Result<CallbackParams, LoopbackError>,
    }

    impl LoopbackSession for FakeSession {
        fn redirect_url(&self) -> String {
            "http://127.0.0.1:5555/callback".into()
        }
        fn wait(self: Box<Self>, _timeout: Duration) -> Result<CallbackParams, LoopbackError> {
            self.outcome
        }
    }

    struct FakeLoopback {
        outcome: Mutex<Option<Result<CallbackParams, LoopbackError>>>,
        bind_fails: bool,
    }

    impl FakeLoopback {
        fn returning(outcome: Result<CallbackParams, LoopbackError>) -> Self {
            Self {
                outcome: Mutex::new(Some(outcome)),
                bind_fails: false,
            }
        }
    }

    impl LoopbackReceiver for FakeLoopback {
        fn bind(&self) -> Result<Box<dyn LoopbackSession>, LoopbackError> {
            if self.bind_fails {
                return Err(LoopbackError::Io("no port".into()));
            }
            let outcome = self.outcome.lock().unwrap().take().unwrap();
            Ok(Box::new(FakeSession { outcome }))
        }
    }

    struct FakePkce;

    impl PkceSource for FakePkce {
        fn state(&self) -> String {
            "state-1".into()
        }
        fn verifier(&self) -> String {
            "verifier-1".into()
        }
        fn challenge(&self, verifier: &str) -> String {
            format!("challenge-of-{verifier}")
        }
    }

    #[derive(Default)]
    struct FakeOpener {
        opened: Mutex<Vec<String>>,
        fail: bool,
    }

    impl WebPageOpener for FakeOpener {
        fn open_page(&self, url: &str) -> Result<(), AppError> {
            if self.fail {
                return Err(AppError::Read {
                    location: url.into(),
                    detail: "no browser".into(),
                });
            }
            self.opened.lock().unwrap().push(url.into());
            Ok(())
        }
    }

    struct FakeAvatars(Option<String>);

    impl AvatarFetcher for FakeAvatars {
        fn fetch(&self, _url: &str) -> Option<String> {
            self.0.clone()
        }
    }

    struct Rig {
        api: Arc<FakeApi>,
        store: Arc<FakeStore>,
        opener: Arc<FakeOpener>,
        service: CommunityAuthService,
    }

    fn callback(code: Option<&str>, state: Option<&str>, error: Option<&str>) -> CallbackParams {
        CallbackParams {
            code: code.map(Into::into),
            state: state.map(Into::into),
            error: error.map(Into::into),
        }
    }

    fn rig(store: FakeStore, loopback: FakeLoopback, opener: FakeOpener, api: FakeApi) -> Rig {
        let api = Arc::new(api);
        let store = Arc::new(store);
        let opener = Arc::new(opener);
        let service = CommunityAuthService::new(
            api.clone(),
            store.clone(),
            Arc::new(loopback),
            Arc::new(FakePkce),
            opener.clone(),
            Arc::new(FakeAvatars(Some("data:image/png;base64,AA==".into()))),
        );
        Rig {
            api,
            store,
            opener,
            service,
        }
    }

    fn signing_in(params: CallbackParams) -> Rig {
        rig(
            FakeStore::default(),
            FakeLoopback::returning(Ok(params)),
            FakeOpener::default(),
            FakeApi::default(),
        )
    }

    fn idle(store: FakeStore, api: FakeApi) -> Rig {
        rig(
            store,
            FakeLoopback::returning(Err(LoopbackError::Timeout)),
            FakeOpener::default(),
            api,
        )
    }

    // Scenario: Signing in with Discord
    #[test]
    fn signing_in_opens_the_browser_exchanges_the_code_and_stores_the_token() {
        let rig = signing_in(callback(Some("otc-1"), Some("state-1"), None));
        let status = rig.service.sign_in().unwrap();
        assert_eq!(
            status,
            CommunityStatus::SignedIn(CommunityProfile {
                display_name: "rider".into(),
                avatar_data_url: Some("data:image/png;base64,AA==".into()),
            })
        );
        assert_eq!(
            rig.api.starts.lock().unwrap().as_slice(),
            [(
                "http://127.0.0.1:5555/callback".to_string(),
                "state-1".to_string(),
                "challenge-of-verifier-1".to_string()
            )]
        );
        assert_eq!(rig.opener.opened.lock().unwrap().len(), 1);
        assert_eq!(
            rig.api.exchanged.lock().unwrap().as_slice(),
            [("otc-1".to_string(), "verifier-1".to_string())]
        );
        assert_eq!(
            rig.store.token.lock().unwrap().as_ref().unwrap().expose(),
            TOKEN
        );
    }

    #[test]
    fn a_missing_avatar_leaves_the_profile_without_one() {
        let api = FakeApi::default();
        *api.exchange.lock().unwrap() = Some(Ok((
            SessionToken::new(TOKEN),
            ServerUser {
                display_name: "rider".into(),
                avatar_url: None,
            },
        )));
        let rig = rig(
            FakeStore::default(),
            FakeLoopback::returning(Ok(callback(Some("c"), Some("state-1"), None))),
            FakeOpener::default(),
            api,
        );
        assert_eq!(
            rig.service.sign_in().unwrap(),
            CommunityStatus::SignedIn(CommunityProfile {
                display_name: "rider".into(),
                avatar_data_url: None,
            })
        );
    }

    #[test]
    fn a_state_mismatch_fails_without_exchanging_or_storing() {
        let rig = signing_in(callback(Some("otc-1"), Some("other"), None));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
        assert!(rig.api.exchanged.lock().unwrap().is_empty());
        assert!(!rig.store.has_token());
    }

    #[test]
    fn a_missing_state_or_code_fails() {
        let rig = signing_in(callback(Some("otc-1"), None, None));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
        let rig = signing_in(callback(None, Some("state-1"), None));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
    }

    #[test]
    fn access_denied_maps_to_sign_in_denied() {
        let rig = signing_in(callback(None, Some("state-1"), Some("access_denied")));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInDenied));
        assert!(!rig.store.has_token());
    }

    #[test]
    fn any_other_error_param_fails() {
        let rig = signing_in(callback(None, Some("state-1"), Some("sign_in_failed")));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
    }

    #[test]
    fn a_denial_with_a_wrong_state_is_not_trusted() {
        let rig = signing_in(callback(None, Some("forged"), Some("access_denied")));
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
    }

    #[test]
    fn a_timeout_maps_to_sign_in_timeout() {
        let rig = idle(FakeStore::default(), FakeApi::default());
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInTimeout));
        assert!(!rig.store.has_token());
    }

    #[test]
    fn a_loopback_io_error_fails() {
        let rig = rig(
            FakeStore::default(),
            FakeLoopback::returning(Err(LoopbackError::Io("x".into()))),
            FakeOpener::default(),
            FakeApi::default(),
        );
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
    }

    #[test]
    fn a_loopback_bind_failure_fails() {
        let mut loopback = FakeLoopback::returning(Err(LoopbackError::Timeout));
        loopback.bind_fails = true;
        let rig = rig(
            FakeStore::default(),
            loopback,
            FakeOpener::default(),
            FakeApi::default(),
        );
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
        assert!(rig.opener.opened.lock().unwrap().is_empty());
    }

    #[test]
    fn a_browser_that_cannot_open_fails() {
        let rig = rig(
            FakeStore::default(),
            FakeLoopback::returning(Err(LoopbackError::Timeout)),
            FakeOpener {
                fail: true,
                ..Default::default()
            },
            FakeApi::default(),
        );
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
    }

    #[test]
    fn a_rejected_exchange_fails_and_stores_nothing() {
        let api = FakeApi::default();
        *api.exchange.lock().unwrap() = Some(Err(CommunityApiError::Protocol("400".into())));
        let rig = rig(
            FakeStore::default(),
            FakeLoopback::returning(Ok(callback(Some("c"), Some("state-1"), None))),
            FakeOpener::default(),
            api,
        );
        assert_eq!(rig.service.sign_in(), Err(CommunityError::SignInFailed));
        assert!(!rig.store.has_token());
    }

    #[test]
    fn an_unreachable_server_during_the_exchange_is_offline() {
        let api = FakeApi::default();
        *api.exchange.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".into())));
        let rig = rig(
            FakeStore::default(),
            FakeLoopback::returning(Ok(callback(Some("c"), Some("state-1"), None))),
            FakeOpener::default(),
            api,
        );
        assert_eq!(rig.service.sign_in(), Err(CommunityError::Offline));
    }

    #[test]
    fn the_timeout_can_be_overridden() {
        let rig = idle(FakeStore::default(), FakeApi::default());
        let service = rig.service.with_timeout(Duration::from_millis(5));
        assert_eq!(service.sign_in(), Err(CommunityError::SignInTimeout));
    }

    // Scenario: Signing out revokes the session
    #[test]
    fn signing_out_revokes_on_the_server_and_deletes_the_token() {
        let rig = idle(FakeStore::holding(TOKEN), FakeApi::default());
        rig.service.sign_out().unwrap();
        assert_eq!(rig.api.logouts.lock().unwrap().as_slice(), [TOKEN]);
        assert!(!rig.store.has_token());
    }

    #[test]
    fn signing_out_offline_still_deletes_the_token() {
        let api = FakeApi::default();
        *api.logout.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".into())));
        let rig = idle(FakeStore::holding(TOKEN), api);
        assert_eq!(rig.service.sign_out(), Ok(()));
        assert!(!rig.store.has_token());
    }

    #[test]
    fn signing_out_without_a_session_is_fine() {
        let rig = idle(FakeStore::default(), FakeApi::default());
        assert_eq!(rig.service.sign_out(), Ok(()));
        assert!(rig.api.logouts.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failing_credential_store_surfaces_on_sign_out() {
        let store = FakeStore {
            fail_load: true,
            ..Default::default()
        };
        let rig = idle(store, FakeApi::default());
        assert!(matches!(
            rig.service.sign_out(),
            Err(CommunityError::Credential(_))
        ));
    }

    // Scenario: An expired session
    #[test]
    fn a_rejected_session_deletes_the_token_and_is_signed_out_without_error() {
        let api = FakeApi::default();
        *api.me.lock().unwrap() = Some(Err(CommunityApiError::Unauthorized));
        let rig = idle(FakeStore::holding(TOKEN), api);
        assert_eq!(rig.service.status(), Ok(CommunityStatus::SignedOut));
        assert!(!rig.store.has_token());
    }

    // Scenario: Server unreachable
    #[test]
    fn an_unreachable_server_is_offline_and_keeps_the_token() {
        let api = FakeApi::default();
        *api.me.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".into())));
        let rig = idle(FakeStore::holding(TOKEN), api);
        assert_eq!(rig.service.status(), Ok(CommunityStatus::Offline));
        assert!(rig.store.has_token());
    }

    #[test]
    fn a_malformed_server_answer_is_offline_and_keeps_the_token() {
        let api = FakeApi::default();
        *api.me.lock().unwrap() = Some(Err(CommunityApiError::Protocol("junk".into())));
        let rig = idle(FakeStore::holding(TOKEN), api);
        assert_eq!(rig.service.status(), Ok(CommunityStatus::Offline));
        assert!(rig.store.has_token());
    }

    #[test]
    fn no_stored_token_is_signed_out_without_asking_the_server() {
        let rig = idle(FakeStore::default(), FakeApi::default());
        assert_eq!(rig.service.status(), Ok(CommunityStatus::SignedOut));
    }

    #[test]
    fn a_valid_session_is_signed_in_with_the_profile() {
        let rig = idle(FakeStore::holding(TOKEN), FakeApi::default());
        assert_eq!(
            rig.service.status(),
            Ok(CommunityStatus::SignedIn(CommunityProfile {
                display_name: "rider".into(),
                avatar_data_url: Some("data:image/png;base64,AA==".into()),
            }))
        );
    }

    #[test]
    fn a_failing_credential_store_surfaces_on_status() {
        let store = FakeStore {
            fail_load: true,
            ..Default::default()
        };
        let rig = idle(store, FakeApi::default());
        assert!(matches!(
            rig.service.status(),
            Err(CommunityError::Credential(_))
        ));
    }

    // Scenario: Secrets stay out of logs and errors
    #[test]
    fn the_token_never_appears_in_debug_output_or_error_messages() {
        let token = SessionToken::new(TOKEN);
        assert!(!format!("{token:?}").contains(TOKEN));
        let errors = [
            CommunityError::Offline,
            CommunityError::SignInFailed,
            CommunityError::SignInTimeout,
            CommunityError::SignInDenied,
        ];
        for error in errors {
            assert!(!format!("{error:?} {error}").contains(TOKEN));
        }
        let api = FakeApi::default();
        *api.me.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".into())));
        let rig = idle(FakeStore::holding(TOKEN), api);
        let status = rig.service.status();
        assert!(!format!("{status:?}").contains(TOKEN));
    }
}
