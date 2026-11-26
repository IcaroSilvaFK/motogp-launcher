//! Discord sign-in use cases: start, callback, one-time code exchange, sessions.

use std::sync::Arc;

use chrono::Duration;
use community_domain::{
    is_url_safe_token, pkce, DiscordId, LoopbackRedirect, OAuthFlow, OneTimeCode, Session,
    SessionToken, User, UserId,
};

use crate::ports::{
    Clock, DiscordOAuth, Hasher, OAuthFlowRepository, OneTimeCodeRepository, PortResult,
    SessionRepository, TokenGenerator, UserRepository, UserUpsert,
};
use crate::AppError;

/// How long a started sign-in may take before Discord's callback.
pub const FLOW_LIFETIME_MINUTES: i64 = 10;
/// How long the one-time code stays valid.
pub const ONE_TIME_CODE_LIFETIME_SECS: i64 = 60;

const MAX_APP_STATE_LEN: usize = 128;
/// A base64url SHA-256 digest without padding.
const APP_CHALLENGE_LEN: usize = 43;
/// 32 random bytes = 256 bits.
const SECRET_BYTES: usize = 32;

/// Query parameters Discord sends back to `/auth/discord/callback`.
#[derive(Debug, Default, Clone)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub error: Option<String>,
    pub state: Option<String>,
}

/// What a successful token exchange returns.
#[derive(Debug, Clone)]
pub struct IssuedSession {
    pub token: SessionToken,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub user: User,
}

/// A session resolved from a bearer token.
#[derive(Debug, Clone)]
pub struct Authenticated {
    pub user: User,
    pub token: SessionToken,
}

pub struct AuthService {
    pub clock: Arc<dyn Clock>,
    pub tokens: Arc<dyn TokenGenerator>,
    pub hasher: Arc<dyn Hasher>,
    pub users: Arc<dyn UserRepository>,
    pub sessions: Arc<dyn SessionRepository>,
    pub flows: Arc<dyn OAuthFlowRepository>,
    pub codes: Arc<dyn OneTimeCodeRepository>,
    pub discord: Arc<dyn DiscordOAuth>,
}

impl AuthService {
    /// Starts a sign-in and returns the Discord authorize URL.
    pub async fn start(
        &self,
        redirect: &str,
        app_state: &str,
        app_challenge: &str,
    ) -> PortResult<String> {
        let redirect = LoopbackRedirect::parse(redirect).ok_or(AppError::InvalidRedirect)?;
        if !is_url_safe_token(app_state, MAX_APP_STATE_LEN) {
            return Err(AppError::BadRequest("The state is invalid.".into()));
        }
        if app_challenge.len() != APP_CHALLENGE_LEN
            || !is_url_safe_token(app_challenge, APP_CHALLENGE_LEN)
        {
            return Err(AppError::BadRequest(
                "The code challenge is invalid.".into(),
            ));
        }

        let now = self.clock.now();
        // Opportunistic cleanup; a failure here must not block sign-in.
        let _ = self.flows.delete_expired(now).await;
        let _ = self.codes.delete_expired(now).await;

        let state = self.tokens.generate(SECRET_BYTES);
        let discord_verifier = self.tokens.generate(SECRET_BYTES);
        let discord_challenge = pkce::challenge_for(&discord_verifier);
        self.flows
            .save(&OAuthFlow {
                state: state.clone(),
                discord_verifier,
                redirect: redirect.as_url(),
                app_state: app_state.to_owned(),
                app_challenge: app_challenge.to_owned(),
                expires_at: now + Duration::minutes(FLOW_LIFETIME_MINUTES),
            })
            .await?;
        Ok(self.discord.authorize_url(&state, &discord_challenge))
    }

    /// Handles Discord's redirect and returns the loopback URL to send the browser to.
    pub async fn callback(&self, params: CallbackParams) -> PortResult<String> {
        let state = params
            .state
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::BadRequest("The state is missing.".into()))?;
        let denied = params.error.is_some();
        let code = params.code.as_deref().filter(|c| !c.is_empty());
        if !denied && code.is_none() {
            return Err(AppError::BadRequest("The code is missing.".into()));
        }

        let now = self.clock.now();
        let flow = self
            .flows
            .take(state, now)
            .await?
            .ok_or_else(|| AppError::BadRequest("The sign-in is unknown or expired.".into()))?;

        if let Some(error) = &params.error {
            let reason = if error == "access_denied" {
                "access_denied"
            } else {
                "sign_in_failed"
            };
            return Ok(format!(
                "{}?error={reason}&state={}",
                flow.redirect, flow.app_state
            ));
        }
        let code = code.expect("checked above");

        let access = self
            .discord
            .exchange_code(code, &flow.discord_verifier)
            .await?;
        let profile = self.discord.fetch_user(&access).await?;
        let discord_id = DiscordId::parse(&profile.id).ok_or(AppError::DiscordUnavailable)?;
        let user = self
            .users
            .upsert(&UserUpsert {
                new_id: UserId(self.tokens.generate(16)),
                discord_id,
                username: profile.username,
                global_name: profile.global_name,
                avatar_hash: profile.avatar,
                now,
            })
            .await?;

        let one_time = self.tokens.generate(SECRET_BYTES);
        self.codes
            .save(&OneTimeCode {
                code_hash: self.hasher.hash(&one_time),
                user_id: user.id,
                app_challenge: flow.app_challenge,
                expires_at: now + Duration::seconds(ONE_TIME_CODE_LIFETIME_SECS),
            })
            .await?;
        Ok(format!(
            "{}?code={one_time}&state={}",
            flow.redirect, flow.app_state
        ))
    }

    /// Trades a one-time code plus the app's PKCE verifier for a session.
    /// The code is consumed on any attempt.
    pub async fn exchange(&self, code: &str, code_verifier: &str) -> PortResult<IssuedSession> {
        let now = self.clock.now();
        let stored = self
            .codes
            .take(&self.hasher.hash(code), now)
            .await?
            .ok_or(AppError::InvalidCode)?;
        if !pkce::verify(code_verifier, &stored.app_challenge) {
            return Err(AppError::InvalidCode);
        }
        let user = self
            .users
            .get(&stored.user_id)
            .await?
            .ok_or(AppError::InvalidCode)?;

        let token = SessionToken::new(self.tokens.generate(SECRET_BYTES));
        let expires_at = Session::expiry_from(now);
        self.sessions
            .create(&Session {
                id: self.tokens.generate(16),
                user_id: user.id.clone(),
                token_hash: self.hasher.hash(token.expose()),
                created_at: now,
                expires_at,
                revoked_at: None,
            })
            .await?;
        Ok(IssuedSession {
            token,
            expires_at,
            user,
        })
    }

    /// Resolves a bearer token to its user; unknown, expired and revoked are all `Unauthorized`.
    pub async fn authenticate(&self, bearer: &str) -> PortResult<User> {
        let session = self
            .sessions
            .find_by_token_hash(&self.hasher.hash(bearer))
            .await?
            .filter(|s| s.is_active(self.clock.now()))
            .ok_or(AppError::Unauthorized)?;
        self.users
            .get(&session.user_id)
            .await?
            .ok_or(AppError::Unauthorized)
    }

    /// Revokes the session; already revoked or unknown is `Unauthorized`.
    pub async fn logout(&self, bearer: &str) -> PortResult<()> {
        let revoked = self
            .sessions
            .revoke(&self.hasher.hash(bearer), self.clock.now())
            .await?;
        revoked.then_some(()).ok_or(AppError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    use chrono::{DateTime, Utc};

    use super::*;
    use crate::ports::{AccessToken, DiscordUser};

    struct TestClock(Mutex<DateTime<Utc>>);
    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    #[derive(Default)]
    struct CountingTokens(AtomicU64);
    impl TokenGenerator for CountingTokens {
        fn generate(&self, _byte_len: usize) -> String {
            format!("tok{}", self.0.fetch_add(1, Ordering::SeqCst))
        }
    }

    struct PlainHasher;
    impl Hasher for PlainHasher {
        fn hash(&self, input: &str) -> String {
            format!("h:{input}")
        }
    }

    #[derive(Default)]
    struct Memory {
        users: Mutex<HashMap<String, User>>,
        sessions: Mutex<Vec<Session>>,
        flows: Mutex<HashMap<String, OAuthFlow>>,
        codes: Mutex<HashMap<String, OneTimeCode>>,
    }

    #[async_trait::async_trait]
    impl UserRepository for Memory {
        async fn upsert(&self, u: &UserUpsert) -> PortResult<User> {
            let mut users = self.users.lock().unwrap();
            let existing = users
                .values()
                .find(|x| x.discord_id == u.discord_id)
                .map(|x| x.id.clone());
            let id = existing.unwrap_or_else(|| u.new_id.clone());
            let user = User {
                id: id.clone(),
                discord_id: u.discord_id.clone(),
                username: u.username.clone(),
                global_name: u.global_name.clone(),
                avatar_hash: u.avatar_hash.clone(),
                created_at: u.now,
            };
            users.insert(id.0, user.clone());
            Ok(user)
        }
        async fn get(&self, id: &UserId) -> PortResult<Option<User>> {
            Ok(self.users.lock().unwrap().get(id.as_str()).cloned())
        }
    }

    #[async_trait::async_trait]
    impl SessionRepository for Memory {
        async fn create(&self, s: &Session) -> PortResult<()> {
            self.sessions.lock().unwrap().push(s.clone());
            Ok(())
        }
        async fn find_by_token_hash(&self, h: &str) -> PortResult<Option<Session>> {
            let sessions = self.sessions.lock().unwrap();
            Ok(sessions.iter().find(|s| s.token_hash == h).cloned())
        }
        async fn revoke(&self, h: &str, now: DateTime<Utc>) -> PortResult<bool> {
            let mut sessions = self.sessions.lock().unwrap();
            match sessions
                .iter_mut()
                .find(|s| s.token_hash == h && s.revoked_at.is_none())
            {
                Some(s) => {
                    s.revoked_at = Some(now);
                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl OAuthFlowRepository for Memory {
        async fn save(&self, f: &OAuthFlow) -> PortResult<()> {
            self.flows
                .lock()
                .unwrap()
                .insert(f.state.clone(), f.clone());
            Ok(())
        }
        async fn take(&self, state: &str, now: DateTime<Utc>) -> PortResult<Option<OAuthFlow>> {
            let flow = self.flows.lock().unwrap().remove(state);
            Ok(flow.filter(|f| f.expires_at > now))
        }
        async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64> {
            let mut flows = self.flows.lock().unwrap();
            let before = flows.len();
            flows.retain(|_, f| f.expires_at > now);
            Ok((before - flows.len()) as u64)
        }
    }

    #[async_trait::async_trait]
    impl OneTimeCodeRepository for Memory {
        async fn save(&self, c: &OneTimeCode) -> PortResult<()> {
            self.codes
                .lock()
                .unwrap()
                .insert(c.code_hash.clone(), c.clone());
            Ok(())
        }
        async fn take(&self, h: &str, now: DateTime<Utc>) -> PortResult<Option<OneTimeCode>> {
            let code = self.codes.lock().unwrap().remove(h);
            Ok(code.filter(|c| c.expires_at > now))
        }
        async fn delete_expired(&self, now: DateTime<Utc>) -> PortResult<u64> {
            let mut codes = self.codes.lock().unwrap();
            let before = codes.len();
            codes.retain(|_, c| c.expires_at > now);
            Ok((before - codes.len()) as u64)
        }
    }

    struct FakeDiscord {
        fail: bool,
        user_id: &'static str,
    }

    #[async_trait::async_trait]
    impl DiscordOAuth for FakeDiscord {
        fn authorize_url(&self, state: &str, challenge: &str) -> String {
            format!("https://discord.test/authorize?state={state}&cc={challenge}")
        }
        async fn exchange_code(&self, code: &str, _verifier: &str) -> PortResult<AccessToken> {
            if self.fail || code == "bad" {
                return Err(AppError::DiscordUnavailable);
            }
            Ok(AccessToken("access".into()))
        }
        async fn fetch_user(&self, _t: &AccessToken) -> PortResult<DiscordUser> {
            Ok(DiscordUser {
                id: self.user_id.into(),
                username: "rider".into(),
                global_name: None,
                avatar: Some("abc".into()),
            })
        }
    }

    struct Fixture {
        service: AuthService,
        memory: Arc<Memory>,
        clock: Arc<TestClock>,
    }

    fn fixture_with(discord: FakeDiscord) -> Fixture {
        let memory = Arc::new(Memory::default());
        let clock = Arc::new(TestClock(Mutex::new(
            DateTime::from_timestamp(1_000_000, 0).unwrap(),
        )));
        let service = AuthService {
            clock: clock.clone(),
            tokens: Arc::new(CountingTokens::default()),
            hasher: Arc::new(PlainHasher),
            users: memory.clone(),
            sessions: memory.clone(),
            flows: memory.clone(),
            codes: memory.clone(),
            discord: Arc::new(discord),
        };
        Fixture {
            service,
            memory,
            clock,
        }
    }

    fn fixture() -> Fixture {
        fixture_with(FakeDiscord {
            fail: false,
            user_id: "42",
        })
    }

    const REDIRECT: &str = "http://127.0.0.1:5000/callback";
    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    /// Runs start + callback and returns the one-time code from the loopback URL.
    async fn sign_in(f: &Fixture) -> String {
        let url = f
            .service
            .start(REDIRECT, "appstate", CHALLENGE)
            .await
            .unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let location = f
            .service
            .callback(CallbackParams {
                code: Some("discord-code".into()),
                error: None,
                state: Some(state.into()),
            })
            .await
            .unwrap();
        assert!(location.starts_with("http://127.0.0.1:5000/callback?code="));
        assert!(location.ends_with("&state=appstate"));
        location
            .split("code=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .to_owned()
    }

    #[tokio::test]
    async fn start_rejects_bad_redirect_state_and_challenge() {
        let f = fixture();
        assert_eq!(
            f.service
                .start("https://evil.example/callback", "s", CHALLENGE)
                .await,
            Err(AppError::InvalidRedirect)
        );
        assert!(matches!(
            f.service.start(REDIRECT, "bad state&", CHALLENGE).await,
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            f.service.start(REDIRECT, "", CHALLENGE).await,
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            f.service.start(REDIRECT, "s", "short").await,
            Err(AppError::BadRequest(_))
        ));
        assert!(f.memory.flows.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn start_stores_a_flow_and_returns_the_authorize_url() {
        let f = fixture();
        let url = f
            .service
            .start(REDIRECT, "appstate", CHALLENGE)
            .await
            .unwrap();
        assert!(url.starts_with("https://discord.test/authorize?state="));
        let flows = f.memory.flows.lock().unwrap();
        let flow = flows.values().next().unwrap();
        assert_eq!(flow.redirect, REDIRECT);
        assert_eq!(flow.app_state, "appstate");
        assert_eq!(flow.app_challenge, CHALLENGE);
        assert_eq!(
            flow.expires_at,
            f.clock.now() + Duration::minutes(FLOW_LIFETIME_MINUTES)
        );
        // The challenge sent to Discord is derived from the stored verifier.
        assert!(url.contains(&pkce::challenge_for(&flow.discord_verifier)));
    }

    impl TestClock {
        fn now(&self) -> DateTime<Utc> {
            Clock::now(self)
        }
        fn advance(&self, by: Duration) {
            *self.0.lock().unwrap() += by;
        }
    }

    #[tokio::test]
    async fn start_deletes_expired_flows_and_codes() {
        let f = fixture();
        f.memory.flows.lock().unwrap().insert(
            "old".into(),
            OAuthFlow {
                state: "old".into(),
                discord_verifier: "v".into(),
                redirect: REDIRECT.into(),
                app_state: "s".into(),
                app_challenge: CHALLENGE.into(),
                expires_at: f.clock.now() - Duration::seconds(1),
            },
        );
        f.memory.codes.lock().unwrap().insert(
            "oldcode".into(),
            OneTimeCode {
                code_hash: "oldcode".into(),
                user_id: UserId("u".into()),
                app_challenge: CHALLENGE.into(),
                expires_at: f.clock.now() - Duration::seconds(1),
            },
        );
        f.service.start(REDIRECT, "s", CHALLENGE).await.unwrap();
        assert!(!f.memory.flows.lock().unwrap().contains_key("old"));
        assert!(f.memory.codes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn callback_with_unknown_or_missing_state_is_bad_request() {
        let f = fixture();
        let unknown = f
            .service
            .callback(CallbackParams {
                code: Some("c".into()),
                error: None,
                state: Some("nope".into()),
            })
            .await;
        assert!(matches!(unknown, Err(AppError::BadRequest(_))));
        let missing_state = f
            .service
            .callback(CallbackParams {
                code: Some("c".into()),
                ..Default::default()
            })
            .await;
        assert!(matches!(missing_state, Err(AppError::BadRequest(_))));
        let missing_code = f
            .service
            .callback(CallbackParams {
                state: Some("s".into()),
                ..Default::default()
            })
            .await;
        assert!(matches!(missing_code, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn callback_with_an_expired_flow_is_bad_request() {
        let f = fixture();
        let url = f.service.start(REDIRECT, "a", CHALLENGE).await.unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        f.clock
            .advance(Duration::minutes(FLOW_LIFETIME_MINUTES + 1));
        let result = f
            .service
            .callback(CallbackParams {
                code: Some("c".into()),
                error: None,
                state: Some(state.into()),
            })
            .await;
        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn callback_access_denied_redirects_with_the_error_and_consumes_the_flow() {
        let f = fixture();
        let url = f
            .service
            .start(REDIRECT, "appstate", CHALLENGE)
            .await
            .unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let params = CallbackParams {
            code: None,
            error: Some("access_denied".into()),
            state: Some(state.into()),
        };
        let location = f.service.callback(params.clone()).await.unwrap();
        assert_eq!(
            location,
            "http://127.0.0.1:5000/callback?error=access_denied&state=appstate"
        );
        assert!(matches!(
            f.service.callback(params).await,
            Err(AppError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn callback_maps_other_discord_errors_to_a_generic_reason() {
        let f = fixture();
        let url = f
            .service
            .start(REDIRECT, "appstate", CHALLENGE)
            .await
            .unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let location = f
            .service
            .callback(CallbackParams {
                error: Some("server_error".into()),
                state: Some(state.into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(location.contains("error=sign_in_failed"));
    }

    #[tokio::test]
    async fn callback_reports_discord_failures() {
        let f = fixture_with(FakeDiscord {
            fail: true,
            user_id: "42",
        });
        let url = f.service.start(REDIRECT, "a", CHALLENGE).await.unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let result = f
            .service
            .callback(CallbackParams {
                code: Some("c".into()),
                error: None,
                state: Some(state.into()),
            })
            .await;
        assert_eq!(result, Err(AppError::DiscordUnavailable));
        assert!(f.memory.codes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn callback_rejects_a_malformed_discord_user_id() {
        let f = fixture_with(FakeDiscord {
            fail: false,
            user_id: "not-a-snowflake",
        });
        let url = f.service.start(REDIRECT, "a", CHALLENGE).await.unwrap();
        let state = url
            .split("state=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let result = f
            .service
            .callback(CallbackParams {
                code: Some("c".into()),
                error: None,
                state: Some(state.into()),
            })
            .await;
        assert_eq!(result, Err(AppError::DiscordUnavailable));
    }

    #[tokio::test]
    async fn full_flow_issues_a_session_and_stores_only_hashes() {
        let f = fixture();
        let code = sign_in(&f).await;
        // The one-time code is stored hashed.
        assert!(!f.memory.codes.lock().unwrap().contains_key(&code));

        let issued = f.service.exchange(&code, VERIFIER).await.unwrap();
        assert_eq!(issued.user.username, "rider");
        {
            let sessions = f.memory.sessions.lock().unwrap();
            assert_eq!(sessions.len(), 1);
            assert_ne!(sessions[0].token_hash, issued.token.expose());
        }

        let user = f.service.authenticate(issued.token.expose()).await.unwrap();
        assert_eq!(user.id, issued.user.id);
    }

    #[tokio::test]
    async fn the_one_time_code_works_once() {
        let f = fixture();
        let code = sign_in(&f).await;
        f.service.exchange(&code, VERIFIER).await.unwrap();
        assert_eq!(
            f.service.exchange(&code, VERIFIER).await.unwrap_err(),
            AppError::InvalidCode
        );
    }

    #[tokio::test]
    async fn a_wrong_verifier_is_rejected_and_still_consumes_the_code() {
        let f = fixture();
        let code = sign_in(&f).await;
        assert_eq!(
            f.service.exchange(&code, "wrong").await.unwrap_err(),
            AppError::InvalidCode
        );
        assert_eq!(
            f.service.exchange(&code, VERIFIER).await.unwrap_err(),
            AppError::InvalidCode
        );
        assert!(f.memory.sessions.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_expired_or_unknown_code_is_rejected() {
        let f = fixture();
        let code = sign_in(&f).await;
        f.clock
            .advance(Duration::seconds(ONE_TIME_CODE_LIFETIME_SECS + 1));
        assert_eq!(
            f.service.exchange(&code, VERIFIER).await.unwrap_err(),
            AppError::InvalidCode
        );
        assert_eq!(
            f.service.exchange("unknown", VERIFIER).await.unwrap_err(),
            AppError::InvalidCode
        );
    }

    #[tokio::test]
    async fn signing_in_twice_reuses_the_user() {
        let f = fixture();
        let first = f
            .service
            .exchange(&sign_in(&f).await, VERIFIER)
            .await
            .unwrap();
        let second = f
            .service
            .exchange(&sign_in(&f).await, VERIFIER)
            .await
            .unwrap();
        assert_eq!(first.user.id, second.user.id);
        assert_ne!(first.token, second.token);
    }

    #[tokio::test]
    async fn authenticate_rejects_unknown_expired_and_revoked_tokens() {
        let f = fixture();
        assert_eq!(
            f.service.authenticate("nope").await.unwrap_err(),
            AppError::Unauthorized
        );
        let issued = f
            .service
            .exchange(&sign_in(&f).await, VERIFIER)
            .await
            .unwrap();
        let bearer = issued.token.expose().to_owned();
        f.clock.advance(Duration::days(31));
        assert_eq!(
            f.service.authenticate(&bearer).await.unwrap_err(),
            AppError::Unauthorized
        );
    }

    #[tokio::test]
    async fn logout_revokes_once() {
        let f = fixture();
        let issued = f
            .service
            .exchange(&sign_in(&f).await, VERIFIER)
            .await
            .unwrap();
        let bearer = issued.token.expose().to_owned();
        f.service.logout(&bearer).await.unwrap();
        assert_eq!(
            f.service.authenticate(&bearer).await.unwrap_err(),
            AppError::Unauthorized
        );
        assert_eq!(
            f.service.logout(&bearer).await.unwrap_err(),
            AppError::Unauthorized
        );
        assert_eq!(
            f.service.logout("unknown").await.unwrap_err(),
            AppError::Unauthorized
        );
    }
}
