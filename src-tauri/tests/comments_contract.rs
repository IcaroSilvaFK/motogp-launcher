//! IPC contract of the mod comment commands (#118) through the Tauri mock
//! runtime with fake ports: never the network or the Windows Credential
//! Manager. The session token must never cross IPC.
mod ipc_support;

use ipc_support::{comments_over, Harness};
use motogp_application::{
    ApiComment, ApiCommentsPage, CommentsApiError, CommunityCommentsApi, CredentialError,
    CredentialStore,
};
use motogp_domain::SessionToken;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const TOKEN: &str = "tok-SECRET-123";

fn comment(id: &str, own: bool) -> ApiComment {
    ApiComment {
        id: id.to_string(),
        author_name: "Ada".to_string(),
        author_avatar_url: None,
        body: "Great livery".to_string(),
        created_at_ms: 1_795_688_130_123,
        edited: false,
        own,
    }
}

#[derive(Default)]
struct ScriptedApi {
    page: Mutex<Option<Result<ApiCommentsPage, CommentsApiError>>>,
    write: Mutex<Option<Result<ApiComment, CommentsApiError>>>,
    delete: Mutex<Option<Result<(), CommentsApiError>>>,
    /// The token each call carried (`None` when the call had none).
    tokens: Mutex<Vec<Option<String>>>,
    bodies: Mutex<Vec<String>>,
}

impl ScriptedApi {
    fn see(&self, token: Option<&SessionToken>) {
        self.tokens
            .lock()
            .unwrap()
            .push(token.map(|t| t.expose().to_string()));
    }
    fn calls(&self) -> usize {
        self.tokens.lock().unwrap().len()
    }
}

impl CommunityCommentsApi for ScriptedApi {
    fn list(
        &self,
        _mod_id: &str,
        _cursor: Option<&str>,
        token: Option<&SessionToken>,
    ) -> Result<ApiCommentsPage, CommentsApiError> {
        self.see(token);
        self.page
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(ApiCommentsPage {
                comments: vec![comment("c1", token.is_some())],
                next_cursor: Some("cur2".to_string()),
            }))
    }
    fn post(
        &self,
        _mod_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        self.see(Some(token));
        self.bodies.lock().unwrap().push(body.to_string());
        self.write
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(|| Ok(comment("new", true)))
    }
    fn edit(
        &self,
        _mod_id: &str,
        comment_id: &str,
        body: &str,
        token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        self.see(Some(token));
        self.bodies.lock().unwrap().push(body.to_string());
        self.write.lock().unwrap().take().unwrap_or_else(|| {
            let mut edited = comment(comment_id, true);
            edited.body = body.to_string();
            edited.edited = true;
            Ok(edited)
        })
    }
    fn delete(
        &self,
        _mod_id: &str,
        _comment_id: &str,
        token: &SessionToken,
    ) -> Result<(), CommentsApiError> {
        self.see(Some(token));
        self.delete.lock().unwrap().take().unwrap_or(Ok(()))
    }
}

#[derive(Default)]
struct MemoryStore {
    token: Mutex<Option<SessionToken>>,
    fail_load: bool,
}

impl MemoryStore {
    fn holding(token: &str) -> Arc<Self> {
        Arc::new(Self {
            token: Mutex::new(Some(SessionToken::new(token))),
            fail_load: false,
        })
    }
    fn has_token(&self) -> bool {
        self.token.lock().unwrap().is_some()
    }
}

impl CredentialStore for MemoryStore {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
        if self.fail_load {
            return Err(CredentialError(format!("locked {TOKEN}")));
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

fn harness(api: &Arc<ScriptedApi>, store: &Arc<MemoryStore>) -> Harness {
    Harness::with_comments(comments_over(api.clone(), store.clone()))
}

/// Every answer, success or error, must be free of the token.
fn clean(result: Result<Value, Value>) -> Result<Value, Value> {
    let text = match &result {
        Ok(value) | Err(value) => value.to_string(),
    };
    assert!(!text.contains(TOKEN), "token leaked over IPC: {text}");
    result
}

fn invoke(h: &Harness, cmd: &str, body: Value) -> Result<Value, Value> {
    clean(h.invoke(cmd, body))
}

#[test]
fn listing_signed_out_returns_a_camel_case_page_and_sends_no_token() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &Arc::default());
    let page = invoke(
        &h,
        "community_comments_list",
        json!({ "modId": "red-livery" }),
    )
    .unwrap();
    assert_eq!(
        page,
        json!({
            "comments": [{
                "id": "c1",
                "authorName": "Ada",
                "authorAvatarDataUrl": null,
                "body": "Great livery",
                "createdAtMs": 1_795_688_130_123_i64,
                "edited": false,
                "own": false
            }],
            "nextCursor": "cur2"
        })
    );
    assert_eq!(*api.tokens.lock().unwrap(), vec![None]);
}

#[test]
fn listing_with_a_session_lets_the_backend_use_the_token_for_own_but_never_exposes_it() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    let page = invoke(
        &h,
        "community_comments_list",
        json!({ "modId": "red-livery", "cursor": "cur1" }),
    )
    .unwrap();
    assert_eq!(page["comments"][0]["own"], json!(true));
    assert_eq!(*api.tokens.lock().unwrap(), vec![Some(TOKEN.to_string())]);
}

#[test]
fn listing_survives_a_failing_credential_store() {
    let store = Arc::new(MemoryStore {
        fail_load: true,
        ..MemoryStore::default()
    });
    let h = harness(&Arc::default(), &store);
    assert!(invoke(
        &h,
        "community_comments_list",
        json!({ "modId": "red-livery" })
    )
    .is_ok());
}

#[test]
fn an_unreachable_server_is_the_offline_error() {
    let api = Arc::new(ScriptedApi::default());
    *api.page.lock().unwrap() = Some(Err(CommentsApiError::Unreachable("down".to_string())));
    let h = harness(&api, &Arc::default());
    let error = invoke(
        &h,
        "community_comments_list",
        json!({ "modId": "red-livery" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
}

#[test]
fn a_bad_mod_id_is_invalid_and_never_reaches_the_server() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    let error = invoke(&h, "community_comments_list", json!({ "modId": "Bad/Id" })).unwrap_err();
    assert_eq!(error["code"], json!("comment_invalid"));
    assert_eq!(api.calls(), 0);
}

#[test]
fn posting_signed_in_answers_the_new_comment() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    let created = invoke(
        &h,
        "community_comment_post",
        json!({ "modId": "red-livery", "body": "  Great livery  " }),
    )
    .unwrap();
    assert_eq!(created["id"], json!("new"));
    assert_eq!(created["own"], json!(true));
    assert_eq!(
        *api.bodies.lock().unwrap(),
        vec!["Great livery".to_string()]
    );
}

#[test]
fn writing_signed_out_is_the_signed_out_error_and_makes_no_call() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &Arc::default());
    for (cmd, body) in [
        (
            "community_comment_post",
            json!({ "modId": "m", "body": "x" }),
        ),
        (
            "community_comment_edit",
            json!({ "modId": "m", "commentId": "c", "body": "x" }),
        ),
        (
            "community_comment_delete",
            json!({ "modId": "m", "commentId": "c" }),
        ),
    ] {
        let error = invoke(&h, cmd, body).unwrap_err();
        assert_eq!(error["code"], json!("community_signed_out"), "{cmd}");
    }
    assert_eq!(api.calls(), 0);
}

#[test]
fn a_401_on_a_write_forgets_the_session_and_reports_signed_out() {
    let api = Arc::new(ScriptedApi::default());
    *api.write.lock().unwrap() = Some(Err(CommentsApiError::Unauthorized));
    let store = MemoryStore::holding(TOKEN);
    let h = harness(&api, &store);
    let error = invoke(
        &h,
        "community_comment_post",
        json!({ "modId": "m", "body": "x" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("community_signed_out"));
    assert!(!store.has_token());
}

#[test]
fn the_write_limit_carries_the_wait_in_seconds() {
    let api = Arc::new(ScriptedApi::default());
    *api.write.lock().unwrap() = Some(Err(CommentsApiError::RateLimited {
        retry_after_secs: 12,
    }));
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    let error = invoke(
        &h,
        "community_comment_post",
        json!({ "modId": "m", "body": "x" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("comment_rate_limited"));
    assert!(error["message"].as_str().unwrap().contains("12 seconds"));
}

#[test]
fn an_over_long_or_empty_comment_is_rejected_before_the_server() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    for body in ["a".repeat(2001), "   ".to_string()] {
        let error = invoke(
            &h,
            "community_comment_post",
            json!({ "modId": "m", "body": body }),
        )
        .unwrap_err();
        assert_eq!(error["code"], json!("comment_invalid"));
    }
    assert_eq!(api.calls(), 0);
}

#[test]
fn editing_answers_the_edited_comment() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    let edited = invoke(
        &h,
        "community_comment_edit",
        json!({ "modId": "m", "commentId": "c1", "body": "Nice livery" }),
    )
    .unwrap();
    assert_eq!(edited["id"], json!("c1"));
    assert_eq!(edited["body"], json!("Nice livery"));
    assert_eq!(edited["edited"], json!(true));
}

#[test]
fn someone_elses_comment_is_not_found_for_edit_and_delete() {
    let api = Arc::new(ScriptedApi::default());
    let h = harness(&api, &MemoryStore::holding(TOKEN));
    *api.write.lock().unwrap() = Some(Err(CommentsApiError::NotFound));
    let error = invoke(
        &h,
        "community_comment_edit",
        json!({ "modId": "m", "commentId": "c1", "body": "x" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("comment_not_found"));
    *api.delete.lock().unwrap() = Some(Err(CommentsApiError::NotFound));
    let error = invoke(
        &h,
        "community_comment_delete",
        json!({ "modId": "m", "commentId": "c1" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("comment_not_found"));
}

#[test]
fn deleting_answers_null() {
    let h = harness(&Arc::default(), &MemoryStore::holding(TOKEN));
    assert_eq!(
        invoke(
            &h,
            "community_comment_delete",
            json!({ "modId": "m", "commentId": "c1" })
        )
        .unwrap(),
        Value::Null
    );
}

#[test]
fn a_credential_failure_never_leaks_its_detail_or_the_token() {
    let store = Arc::new(MemoryStore {
        fail_load: true,
        ..MemoryStore::default()
    });
    let h = harness(&Arc::default(), &store);
    let error = invoke(
        &h,
        "community_comment_post",
        json!({ "modId": "m", "body": "x" }),
    )
    .unwrap_err();
    assert_eq!(error["code"], json!("community_signed_out"));
}

#[test]
fn the_default_composition_reads_offline() {
    let h = Harness::new();
    let error = invoke(&h, "community_comments_list", json!({ "modId": "m" })).unwrap_err();
    assert_eq!(error["code"], json!("community_offline"));
}
