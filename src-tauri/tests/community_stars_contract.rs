//! IPC contract of the mod stars commands (#119) through the Tauri mock runtime
//! with fake ports: never the network or the Windows Credential Manager.
mod ipc_support;

use ipc_support::Harness;
use motogp_application::{
    CommunityApiError, CommunityStarsService, CredentialError, CredentialStore, ModStats,
    ModStatsApi,
};
use motogp_domain::SessionToken;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const TOKEN: &str = "tok-SECRET-123";

/// One mod with 10 stars; starring adds one, unstarring takes it back.
struct FakeServer {
    stars: Mutex<u64>,
    offline: bool,
    seen_tokens: Mutex<Vec<Option<String>>>,
}

impl FakeServer {
    fn new(offline: bool) -> Arc<Self> {
        Arc::new(Self {
            stars: Mutex::new(10),
            offline,
            seen_tokens: Mutex::default(),
        })
    }

    fn check(&self, token: Option<&SessionToken>) -> Result<(), CommunityApiError> {
        self.seen_tokens
            .lock()
            .unwrap()
            .push(token.map(|t| t.expose().to_string()));
        if self.offline {
            return Err(CommunityApiError::Unreachable("down".into()));
        }
        match token {
            Some(t) if t.expose() == "stale" => Err(CommunityApiError::Unauthorized),
            _ => Ok(()),
        }
    }
}

impl ModStatsApi for FakeServer {
    fn stats(&self, token: Option<&SessionToken>) -> Result<Vec<ModStats>, CommunityApiError> {
        self.check(None)?;
        Ok(vec![ModStats {
            id: "a-mod".into(),
            stars: *self.stars.lock().unwrap(),
            starred_by_me: token.map(|_| true),
        }])
    }
    fn star(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
        self.check(Some(token))?;
        let mut stars = self.stars.lock().unwrap();
        *stars += 1;
        Ok(ModStats {
            id: id.into(),
            stars: *stars,
            starred_by_me: Some(true),
        })
    }
    fn unstar(&self, token: &SessionToken, id: &str) -> Result<ModStats, CommunityApiError> {
        self.check(Some(token))?;
        let mut stars = self.stars.lock().unwrap();
        *stars -= 1;
        Ok(ModStats {
            id: id.into(),
            stars: *stars,
            starred_by_me: Some(false),
        })
    }
}

struct Store(Option<SessionToken>);

impl CredentialStore for Store {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
        Ok(self.0.clone())
    }
    fn save(&self, _token: &SessionToken) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self) -> Result<(), CredentialError> {
        Ok(())
    }
}

fn harness(server: &Arc<FakeServer>, token: Option<&str>) -> Harness {
    let store = Arc::new(Store(token.map(SessionToken::new)));
    Harness::with_stars(Arc::new(CommunityStarsService::new(server.clone(), store)))
}

fn assert_no_token(value: &Value) {
    assert!(!value.to_string().contains(TOKEN), "{value}");
}

#[test]
fn stats_without_a_session_list_the_counts_without_the_flag() {
    let h = harness(&FakeServer::new(false), None);
    assert_eq!(
        h.invoke("community_mod_stats", json!({})).unwrap(),
        json!({"mods": [{"id": "a-mod", "stars": 10}]})
    );
}

#[test]
fn stats_with_a_session_carry_the_starred_flag_and_no_token() {
    let h = harness(&FakeServer::new(false), Some(TOKEN));
    let stats = h.invoke("community_mod_stats", json!({})).unwrap();
    assert_eq!(
        stats,
        json!({"mods": [{"id": "a-mod", "stars": 10, "starredByMe": true}]})
    );
    assert_no_token(&stats);
}

#[test]
fn stats_of_an_unreachable_server_are_offline() {
    let h = harness(&FakeServer::new(true), None);
    let error = h.invoke("community_mod_stats", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
}

#[test]
fn starring_and_unstarring_answer_the_new_count() {
    let h = harness(&FakeServer::new(false), Some(TOKEN));
    let starred = h.invoke("community_star", json!({"id": "a-mod"})).unwrap();
    assert_eq!(
        starred,
        json!({"id": "a-mod", "stars": 11, "starredByMe": true})
    );
    assert_no_token(&starred);
    assert_eq!(
        h.invoke("community_unstar", json!({"id": "a-mod"}))
            .unwrap(),
        json!({"id": "a-mod", "stars": 10, "starredByMe": false})
    );
}

#[test]
fn starring_while_signed_out_is_refused_without_calling_the_server() {
    let server = FakeServer::new(false);
    let h = harness(&server, None);
    for command in ["community_star", "community_unstar"] {
        let error = h.invoke(command, json!({"id": "a-mod"})).unwrap_err();
        assert_eq!(error["code"], json!("community_signed_out"));
    }
    assert!(server.seen_tokens.lock().unwrap().is_empty());
}

#[test]
fn a_rejected_session_is_signed_out() {
    let h = harness(&FakeServer::new(false), Some("stale"));
    let error = h
        .invoke("community_star", json!({"id": "a-mod"}))
        .unwrap_err();
    assert_eq!(error["code"], json!("community_signed_out"));
}

#[test]
fn starring_against_an_unreachable_server_is_offline() {
    let h = harness(&FakeServer::new(true), Some(TOKEN));
    let error = h
        .invoke("community_star", json!({"id": "a-mod"}))
        .unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
}

#[test]
fn the_default_composition_is_offline() {
    let h = Harness::new();
    let error = h.invoke("community_mod_stats", json!({})).unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
}
