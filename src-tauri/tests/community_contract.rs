//! IPC contract of the community account commands (#122) through the Tauri
//! mock runtime with fake ports: never the network, a browser or the Windows
//! Credential Manager.
mod ipc_support;

use ipc_support::{community_over, Harness};
use motogp_application::{
    CallbackParams, CommunityApi, CommunityApiError, CredentialError, CredentialStore,
    LoopbackError, LoopbackReceiver, LoopbackSession, ServerUser,
};
use motogp_domain::SessionToken;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "tok-SECRET-123";

fn rider() -> ServerUser {
    ServerUser {
        display_name: "rider".to_string(),
        avatar_url: None,
    }
}

#[derive(Default)]
struct ScriptedApi {
    me: Mutex<Option<Result<ServerUser, CommunityApiError>>>,
    exchange: Mutex<Option<Result<(SessionToken, ServerUser), CommunityApiError>>>,
    logout: Mutex<Option<Result<(), CommunityApiError>>>,
    logouts: Mutex<usize>,
}

impl CommunityApi for ScriptedApi {
    fn start_url(&self, _redirect: &str, _state: &str, _challenge: &str) -> String {
        // The harness opener accepts any URL: only the shape matters here.
        "http://127.0.0.1:8787/auth/discord/start?x=1".to_string()
    }
    fn exchange(
        &self,
        _code: &str,
        _verifier: &str,
    ) -> Result<(SessionToken, ServerUser), CommunityApiError> {
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
    fn logout(&self, _token: &SessionToken) -> Result<(), CommunityApiError> {
        *self.logouts.lock().unwrap() += 1;
        self.logout.lock().unwrap().take().unwrap_or(Ok(()))
    }
}

#[derive(Default)]
struct MemoryStore {
    token: Mutex<Option<SessionToken>>,
}

impl MemoryStore {
    fn holding(token: &str) -> Arc<Self> {
        Arc::new(Self {
            token: Mutex::new(Some(SessionToken::new(token))),
        })
    }
    fn has_token(&self) -> bool {
        self.token.lock().unwrap().is_some()
    }
}

impl CredentialStore for MemoryStore {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
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

struct Browser(Result<CallbackParams, LoopbackError>);

struct BrowserSession(Result<CallbackParams, LoopbackError>);

impl LoopbackSession for BrowserSession {
    fn redirect_url(&self) -> String {
        "http://127.0.0.1:5555/callback".to_string()
    }
    fn wait(self: Box<Self>, _timeout: Duration) -> Result<CallbackParams, LoopbackError> {
        self.0
    }
}

impl LoopbackReceiver for Browser {
    fn bind(&self) -> Result<Box<dyn LoopbackSession>, LoopbackError> {
        Ok(Box::new(BrowserSession(self.0.clone())))
    }
}

fn comes_back(code: Option<&str>, state: Option<&str>, error: Option<&str>) -> Browser {
    Browser(Ok(CallbackParams {
        code: code.map(Into::into),
        state: state.map(Into::into),
        error: error.map(Into::into),
    }))
}

fn harness(api: Arc<ScriptedApi>, store: Arc<MemoryStore>, browser: Browser) -> Harness {
    Harness::with_community(community_over(api, store, Arc::new(browser)))
}

fn idle(api: Arc<ScriptedApi>, store: Arc<MemoryStore>) -> Harness {
    harness(api, store, Browser(Err(LoopbackError::Timeout)))
}

fn assert_no_token(value: &Value) {
    assert!(!value.to_string().contains(TOKEN), "{value}");
}

#[test]
fn community_me_without_a_session_is_signed_out() {
    let h = idle(Arc::default(), Arc::default());
    assert_eq!(
        h.invoke("community_me", json!({})).unwrap(),
        json!({ "state": "signed_out" })
    );
}

#[test]
fn community_me_with_a_valid_session_is_signed_in_with_a_camel_case_profile() {
    let h = idle(Arc::default(), MemoryStore::holding(TOKEN));
    let status = h.invoke("community_me", json!({})).unwrap();
    assert_eq!(
        status,
        json!({
            "state": "signed_in",
            "profile": { "displayName": "rider", "avatarDataUrl": null }
        })
    );
    assert_no_token(&status);
}

#[test]
fn community_me_with_an_expired_session_is_signed_out_and_forgets_the_token() {
    let api = Arc::new(ScriptedApi::default());
    *api.me.lock().unwrap() = Some(Err(CommunityApiError::Unauthorized));
    let store = MemoryStore::holding(TOKEN);
    let h = idle(api, store.clone());
    assert_eq!(
        h.invoke("community_me", json!({})).unwrap(),
        json!({ "state": "signed_out" })
    );
    assert!(!store.has_token());
}

#[test]
fn community_me_with_an_unreachable_server_is_offline_and_keeps_the_token() {
    let api = Arc::new(ScriptedApi::default());
    *api.me.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".to_string())));
    let store = MemoryStore::holding(TOKEN);
    let h = idle(api, store.clone());
    assert_eq!(
        h.invoke("community_me", json!({})).unwrap(),
        json!({ "state": "offline" })
    );
    assert!(store.has_token());
}

#[test]
fn community_sign_in_stores_the_session_and_answers_the_profile_without_the_token() {
    let store = Arc::new(MemoryStore::default());
    let h = harness(
        Arc::default(),
        store.clone(),
        comes_back(Some("otc"), Some("state-1"), None),
    );
    let status = h.invoke("community_sign_in", json!({})).unwrap();
    assert_eq!(status["state"], json!("signed_in"));
    assert_eq!(status["profile"]["displayName"], json!("rider"));
    assert_no_token(&status);
    assert!(store.has_token());
}

#[test]
fn community_sign_in_times_out_with_a_stable_code() {
    let store = Arc::new(MemoryStore::default());
    let h = idle(Arc::default(), store.clone());
    let error = h.invoke("community_sign_in", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_sign_in_timeout"));
    assert!(!store.has_token());
}

#[test]
fn community_sign_in_denied_on_discord_has_its_own_code() {
    let h = harness(
        Arc::default(),
        Arc::default(),
        comes_back(None, Some("state-1"), Some("access_denied")),
    );
    let error = h.invoke("community_sign_in", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_sign_in_denied"));
}

#[test]
fn community_sign_in_with_a_wrong_state_fails() {
    let store = Arc::new(MemoryStore::default());
    let h = harness(
        Arc::default(),
        store.clone(),
        comes_back(Some("otc"), Some("forged"), None),
    );
    let error = h.invoke("community_sign_in", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_sign_in_failed"));
    assert!(!store.has_token());
}

#[test]
fn community_sign_in_against_an_unreachable_server_is_offline() {
    let api = Arc::new(ScriptedApi::default());
    *api.exchange.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".to_string())));
    let h = harness(
        api,
        Arc::default(),
        comes_back(Some("otc"), Some("state-1"), None),
    );
    let error = h.invoke("community_sign_in", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
    assert_no_token(&error);
}

#[test]
fn community_sign_out_revokes_and_forgets_the_session() {
    let api = Arc::new(ScriptedApi::default());
    let store = MemoryStore::holding(TOKEN);
    let h = idle(api.clone(), store.clone());
    assert_eq!(
        h.invoke("community_sign_out", json!({})).unwrap(),
        Value::Null
    );
    assert_eq!(*api.logouts.lock().unwrap(), 1);
    assert!(!store.has_token());
}

#[test]
fn community_sign_out_offline_still_forgets_the_session() {
    let api = Arc::new(ScriptedApi::default());
    *api.logout.lock().unwrap() = Some(Err(CommunityApiError::Unreachable("down".to_string())));
    let store = MemoryStore::holding(TOKEN);
    let h = idle(api, store.clone());
    assert_eq!(
        h.invoke("community_sign_out", json!({})).unwrap(),
        Value::Null
    );
    assert!(!store.has_token());
}

#[test]
fn the_default_composition_is_signed_out_and_cannot_sign_in() {
    let h = Harness::new();
    assert_eq!(
        h.invoke("community_me", json!({})).unwrap(),
        json!({ "state": "signed_out" })
    );
    let error = h.invoke("community_sign_in", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_sign_in_failed"));
}
