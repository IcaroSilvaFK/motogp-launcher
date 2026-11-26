//! Use case: mod comments (issue #118). Reads work without an account; writes
//! use the session token from the credential store, which never leaves this
//! layer. Avatars are downloaded here and cached in memory.
use crate::{
    ApiComment, AvatarFetcher, CommentsApiError, CommentsError, CommunityCommentsApi,
    CredentialStore,
};
use motogp_domain::{
    is_valid_catalog_slug, parse_report_details, CommentDraft, CommentsPage, ModComment,
    ReportReason, SessionToken,
};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// How many avatar URLs the in-memory cache keeps.
pub const AVATAR_CACHE_CAPACITY: usize = 256;

/// Avatar URL to `data:` URL (`None` when the download failed, so a broken
/// avatar is not retried on every page). Oldest entry leaves first.
#[derive(Default)]
struct AvatarCache {
    entries: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl AvatarCache {
    fn insert(&mut self, url: String, value: Option<String>) {
        if self.entries.insert(url.clone(), value).is_none() {
            self.order.push_back(url);
            while self.order.len() > AVATAR_CACHE_CAPACITY {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
        }
    }
}

pub struct CommunityCommentsService {
    api: Arc<dyn CommunityCommentsApi>,
    store: Arc<dyn CredentialStore>,
    avatars: Arc<dyn AvatarFetcher>,
    cache: Mutex<AvatarCache>,
}

impl CommunityCommentsService {
    pub fn new(
        api: Arc<dyn CommunityCommentsApi>,
        store: Arc<dyn CredentialStore>,
        avatars: Arc<dyn AvatarFetcher>,
    ) -> Self {
        Self {
            api,
            store,
            avatars,
            cache: Mutex::default(),
        }
    }

    /// One page of comments, oldest first. The stored token (if any) only
    /// makes the server mark `own`; an empty or failing credential store never
    /// fails the read.
    pub fn list(&self, mod_id: &str, cursor: Option<&str>) -> Result<CommentsPage, CommentsError> {
        check_mod_id(mod_id)?;
        let token = self.store.load().ok().flatten();
        let page = self
            .api
            .list(mod_id, cursor, token.as_ref())
            .map_err(read_error)?;
        Ok(CommentsPage {
            comments: page.comments.into_iter().map(|c| self.view(c)).collect(),
            next_cursor: page.next_cursor,
        })
    }

    pub fn post(&self, mod_id: &str, body: &str) -> Result<ModComment, CommentsError> {
        check_mod_id(mod_id)?;
        let draft = parse_draft(body)?;
        let token = self.require_token()?;
        let comment = self
            .api
            .post(mod_id, draft.as_str(), &token)
            .map_err(|e| self.write_error(e))?;
        Ok(self.view(comment))
    }

    pub fn edit(
        &self,
        mod_id: &str,
        comment_id: &str,
        body: &str,
    ) -> Result<ModComment, CommentsError> {
        check_mod_id(mod_id)?;
        let draft = parse_draft(body)?;
        let token = self.require_token()?;
        let comment = self
            .api
            .edit(mod_id, comment_id, draft.as_str(), &token)
            .map_err(|e| self.write_error(e))?;
        Ok(self.view(comment))
    }

    pub fn delete(&self, mod_id: &str, comment_id: &str) -> Result<(), CommentsError> {
        check_mod_id(mod_id)?;
        let token = self.require_token()?;
        self.api
            .delete(mod_id, comment_id, &token)
            .map_err(|e| self.write_error(e))
    }

    /// Reports another member's comment. `details` is trimmed (blank is
    /// dropped) and capped at 500 characters before anything is sent.
    pub fn report(
        &self,
        mod_id: &str,
        comment_id: &str,
        reason: ReportReason,
        details: Option<&str>,
    ) -> Result<(), CommentsError> {
        check_mod_id(mod_id)?;
        let details = parse_report_details(details).map_err(|_| CommentsError::Invalid)?;
        let token = self.require_token()?;
        self.api
            .report(mod_id, comment_id, reason, details.as_deref(), &token)
            .map_err(|e| self.write_error(e))
    }

    fn require_token(&self) -> Result<SessionToken, CommentsError> {
        self.store.load()?.ok_or(CommentsError::SignedOut)
    }

    /// A 401 means the session is dead: forget it, like the account status does.
    fn write_error(&self, error: CommentsApiError) -> CommentsError {
        if error == CommentsApiError::Unauthorized {
            if let Err(error) = self.store.delete() {
                return CommentsError::Credential(error);
            }
            return CommentsError::SignedOut;
        }
        read_error(error)
    }

    fn view(&self, comment: ApiComment) -> ModComment {
        let author_avatar_data_url = comment.author_avatar_url.and_then(|url| self.avatar(&url));
        ModComment {
            id: comment.id,
            author_name: comment.author_name,
            author_avatar_data_url,
            body: comment.body,
            created_at_ms: comment.created_at_ms,
            edited: comment.edited,
            own: comment.own,
        }
    }

    fn avatar(&self, url: &str) -> Option<String> {
        if let Some(hit) = self.lock().entries.get(url) {
            return hit.clone();
        }
        // The download runs outside the lock: a slow avatar must not block others.
        let fetched = self.avatars.fetch(url);
        self.lock().insert(url.to_string(), fetched.clone());
        fetched
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AvatarCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn check_mod_id(mod_id: &str) -> Result<(), CommentsError> {
    if is_valid_catalog_slug(mod_id) {
        Ok(())
    } else {
        Err(CommentsError::Invalid)
    }
}

fn parse_draft(body: &str) -> Result<CommentDraft, CommentsError> {
    CommentDraft::parse(body).map_err(|_| CommentsError::Invalid)
}

fn read_error(error: CommentsApiError) -> CommentsError {
    match error {
        CommentsApiError::Unreachable(_)
        | CommentsApiError::Protocol(_)
        | CommentsApiError::Unauthorized => CommentsError::Offline,
        CommentsApiError::RateLimited { retry_after_secs } => {
            CommentsError::RateLimited { retry_after_secs }
        }
        CommentsApiError::Invalid => CommentsError::Invalid,
        CommentsApiError::NotFound => CommentsError::NotFound,
        CommentsApiError::Banned => CommentsError::Banned,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApiCommentsPage, CredentialError};

    const TOKEN: &str = "tok-SECRET-123";

    fn comment(id: &str, avatar: Option<&str>) -> ApiComment {
        ApiComment {
            id: id.into(),
            author_name: "Ada".into(),
            author_avatar_url: avatar.map(Into::into),
            body: "hi".into(),
            created_at_ms: 1_000,
            edited: false,
            own: false,
        }
    }

    #[derive(Default)]
    struct FakeApi {
        page: Mutex<Option<Result<ApiCommentsPage, CommentsApiError>>>,
        write: Mutex<Option<Result<ApiComment, CommentsApiError>>>,
        delete: Mutex<Option<Result<(), CommentsApiError>>>,
        report: Mutex<Option<Result<(), CommentsApiError>>>,
        /// (comment id, reason, details) per report call.
        reports: Mutex<Vec<(String, String, Option<String>)>>,
        /// (operation, token seen) per call.
        calls: Mutex<Vec<(String, Option<String>)>>,
        sent: Mutex<Vec<String>>,
    }

    impl FakeApi {
        fn record(&self, op: &str, token: Option<&SessionToken>) {
            self.calls
                .lock()
                .unwrap()
                .push((op.into(), token.map(|t| t.expose().to_string())));
        }
    }

    impl CommunityCommentsApi for FakeApi {
        fn list(
            &self,
            _mod_id: &str,
            cursor: Option<&str>,
            token: Option<&SessionToken>,
        ) -> Result<ApiCommentsPage, CommentsApiError> {
            self.record(&format!("list:{}", cursor.unwrap_or("-")), token);
            self.page
                .lock()
                .unwrap()
                .take()
                .unwrap_or(Ok(ApiCommentsPage {
                    comments: vec![],
                    next_cursor: None,
                }))
        }
        fn post(
            &self,
            _mod_id: &str,
            body: &str,
            token: &SessionToken,
        ) -> Result<ApiComment, CommentsApiError> {
            self.record("post", Some(token));
            self.sent.lock().unwrap().push(body.into());
            self.write
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok(comment("new", None)))
        }
        fn edit(
            &self,
            _mod_id: &str,
            comment_id: &str,
            body: &str,
            token: &SessionToken,
        ) -> Result<ApiComment, CommentsApiError> {
            self.record(&format!("edit:{comment_id}"), Some(token));
            self.sent.lock().unwrap().push(body.into());
            self.write
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Ok(comment(comment_id, None)))
        }
        fn delete(
            &self,
            _mod_id: &str,
            comment_id: &str,
            token: &SessionToken,
        ) -> Result<(), CommentsApiError> {
            self.record(&format!("delete:{comment_id}"), Some(token));
            self.delete.lock().unwrap().take().unwrap_or(Ok(()))
        }
        fn report(
            &self,
            _mod_id: &str,
            comment_id: &str,
            reason: ReportReason,
            details: Option<&str>,
            token: &SessionToken,
        ) -> Result<(), CommentsApiError> {
            self.record(&format!("report:{comment_id}"), Some(token));
            self.reports.lock().unwrap().push((
                comment_id.into(),
                reason.as_str().into(),
                details.map(Into::into),
            ));
            self.report.lock().unwrap().take().unwrap_or(Ok(()))
        }
    }

    #[derive(Default)]
    struct FakeStore {
        token: Mutex<Option<SessionToken>>,
        fail_load: bool,
    }

    impl FakeStore {
        fn holding() -> Arc<Self> {
            Arc::new(Self {
                token: Mutex::new(Some(SessionToken::new(TOKEN))),
                fail_load: false,
            })
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

    #[derive(Default)]
    struct CountingAvatars {
        fetched: Mutex<Vec<String>>,
        fail: bool,
    }

    impl AvatarFetcher for CountingAvatars {
        fn fetch(&self, url: &str) -> Option<String> {
            self.fetched.lock().unwrap().push(url.into());
            (!self.fail).then(|| format!("data:image/png;base64,{}", url.len()))
        }
    }

    fn service(
        api: &Arc<FakeApi>,
        store: &Arc<FakeStore>,
        avatars: &Arc<CountingAvatars>,
    ) -> CommunityCommentsService {
        CommunityCommentsService::new(api.clone(), store.clone(), avatars.clone())
    }

    fn setup() -> (Arc<FakeApi>, Arc<FakeStore>, Arc<CountingAvatars>) {
        (
            Arc::default(),
            FakeStore::holding(),
            Arc::new(CountingAvatars::default()),
        )
    }

    #[test]
    fn listing_without_a_stored_token_works_and_sends_none() {
        let (api, _, avatars) = setup();
        let store = Arc::new(FakeStore::default());
        let page = service(&api, &store, &avatars)
            .list("livery", None)
            .unwrap();
        assert!(page.comments.is_empty());
        assert_eq!(api.calls.lock().unwrap()[0], ("list:-".to_string(), None));
    }

    #[test]
    fn listing_survives_a_failing_credential_store() {
        let (api, _, avatars) = setup();
        let store = Arc::new(FakeStore {
            fail_load: true,
            ..FakeStore::default()
        });
        assert!(service(&api, &store, &avatars).list("livery", None).is_ok());
    }

    #[test]
    fn listing_sends_the_stored_token_and_the_cursor_and_maps_the_page() {
        let (api, store, avatars) = setup();
        *api.page.lock().unwrap() = Some(Ok(ApiCommentsPage {
            comments: vec![comment("c1", Some("https://cdn.example/a.png"))],
            next_cursor: Some("next".into()),
        }));
        let page = service(&api, &store, &avatars)
            .list("livery", Some("cur"))
            .unwrap();
        assert_eq!(
            api.calls.lock().unwrap()[0],
            ("list:cur".to_string(), Some(TOKEN.to_string()))
        );
        assert_eq!(page.next_cursor.as_deref(), Some("next"));
        assert_eq!(page.comments[0].id, "c1");
        assert!(page.comments[0]
            .author_avatar_data_url
            .as_deref()
            .unwrap()
            .starts_with("data:image/png"));
    }

    #[test]
    fn an_invalid_mod_id_never_reaches_the_server() {
        let (api, store, avatars) = setup();
        let svc = service(&api, &store, &avatars);
        assert_eq!(svc.list("Bad Id", None), Err(CommentsError::Invalid));
        assert_eq!(svc.post("a/b", "x"), Err(CommentsError::Invalid));
        assert_eq!(svc.edit("", "c", "x"), Err(CommentsError::Invalid));
        assert_eq!(svc.delete("A", "c"), Err(CommentsError::Invalid));
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn an_invalid_draft_is_rejected_before_any_call() {
        let (api, store, avatars) = setup();
        let svc = service(&api, &store, &avatars);
        assert_eq!(svc.post("livery", "  "), Err(CommentsError::Invalid));
        assert_eq!(
            svc.post("livery", &"a".repeat(2001)),
            Err(CommentsError::Invalid)
        );
        assert_eq!(svc.edit("livery", "c", ""), Err(CommentsError::Invalid));
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn posting_sends_the_trimmed_body_with_the_token() {
        let (api, store, avatars) = setup();
        let created = service(&api, &store, &avatars)
            .post("livery", "  Great livery \r\n")
            .unwrap();
        assert_eq!(created.id, "new");
        assert_eq!(*api.sent.lock().unwrap(), vec!["Great livery".to_string()]);
        assert_eq!(
            api.calls.lock().unwrap()[0],
            ("post".to_string(), Some(TOKEN.to_string()))
        );
    }

    #[test]
    fn writes_without_a_session_are_signed_out_and_make_no_call() {
        let (api, _, avatars) = setup();
        let store = Arc::new(FakeStore::default());
        let svc = service(&api, &store, &avatars);
        assert_eq!(svc.post("livery", "x"), Err(CommentsError::SignedOut));
        assert_eq!(svc.edit("livery", "c", "x"), Err(CommentsError::SignedOut));
        assert_eq!(svc.delete("livery", "c"), Err(CommentsError::SignedOut));
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn a_credential_failure_on_a_write_is_reported() {
        let (api, _, avatars) = setup();
        let store = Arc::new(FakeStore {
            fail_load: true,
            ..FakeStore::default()
        });
        assert!(matches!(
            service(&api, &store, &avatars).post("livery", "x"),
            Err(CommentsError::Credential(_))
        ));
    }

    #[test]
    fn a_401_on_a_write_forgets_the_token_and_signs_out() {
        for op in 0..3 {
            let (api, store, avatars) = setup();
            *api.write.lock().unwrap() = Some(Err(CommentsApiError::Unauthorized));
            *api.delete.lock().unwrap() = Some(Err(CommentsApiError::Unauthorized));
            let svc = service(&api, &store, &avatars);
            let result = match op {
                0 => svc.post("livery", "x").map(drop),
                1 => svc.edit("livery", "c", "x").map(drop),
                _ => svc.delete("livery", "c"),
            };
            assert_eq!(result, Err(CommentsError::SignedOut));
            assert!(!store.has_token());
        }
    }

    #[test]
    fn reporting_sends_the_reason_and_trimmed_details_with_the_token() {
        let (api, store, avatars) = setup();
        let svc = service(&api, &store, &avatars);
        svc.report(
            "livery",
            "c9",
            ReportReason::OffTopic,
            Some("  not a livery  "),
        )
        .unwrap();
        svc.report("livery", "c9", ReportReason::Spam, Some("   "))
            .unwrap();
        assert_eq!(
            *api.reports.lock().unwrap(),
            vec![
                (
                    "c9".to_string(),
                    "off_topic".to_string(),
                    Some("not a livery".to_string())
                ),
                ("c9".to_string(), "spam".to_string(), None),
            ]
        );
        assert_eq!(
            api.calls.lock().unwrap()[0],
            ("report:c9".to_string(), Some(TOKEN.to_string()))
        );
    }

    #[test]
    fn reporting_rejects_bad_input_before_any_request() {
        let (api, store, avatars) = setup();
        let svc = service(&api, &store, &avatars);
        let long = "a".repeat(501);
        assert_eq!(
            svc.report("livery", "c", ReportReason::Other, Some(&long)),
            Err(CommentsError::Invalid)
        );
        assert_eq!(
            svc.report("Bad Id", "c", ReportReason::Other, None),
            Err(CommentsError::Invalid)
        );
        assert!(api.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn reporting_signed_out_fails_without_a_request_and_a_401_forgets_the_session() {
        let (api, _, avatars) = setup();
        let empty = Arc::new(FakeStore::default());
        assert_eq!(
            service(&api, &empty, &avatars).report("livery", "c", ReportReason::Spam, None),
            Err(CommentsError::SignedOut)
        );
        assert!(api.calls.lock().unwrap().is_empty());
        let (api, store, avatars) = setup();
        *api.report.lock().unwrap() = Some(Err(CommentsApiError::Unauthorized));
        assert_eq!(
            service(&api, &store, &avatars).report("livery", "c", ReportReason::Spam, None),
            Err(CommentsError::SignedOut)
        );
        assert!(!store.has_token());
    }

    #[test]
    fn a_banned_report_surfaces_as_banned_and_keeps_the_session() {
        let (api, store, avatars) = setup();
        *api.report.lock().unwrap() = Some(Err(CommentsApiError::Banned));
        assert_eq!(
            service(&api, &store, &avatars).report("livery", "c", ReportReason::Abuse, None),
            Err(CommentsError::Banned)
        );
        assert!(store.has_token());
    }

    #[test]
    fn server_failures_map_to_the_use_case_errors() {
        let cases = [
            (
                CommentsApiError::RateLimited {
                    retry_after_secs: 12,
                },
                CommentsError::RateLimited {
                    retry_after_secs: 12,
                },
            ),
            (CommentsApiError::NotFound, CommentsError::NotFound),
            (CommentsApiError::Banned, CommentsError::Banned),
            (CommentsApiError::Invalid, CommentsError::Invalid),
            (
                CommentsApiError::Unreachable("x".into()),
                CommentsError::Offline,
            ),
            (
                CommentsApiError::Protocol("x".into()),
                CommentsError::Offline,
            ),
        ];
        for (api_error, expected) in cases {
            let (api, store, avatars) = setup();
            *api.write.lock().unwrap() = Some(Err(api_error.clone()));
            *api.page.lock().unwrap() = Some(Err(api_error));
            let svc = service(&api, &store, &avatars);
            assert_eq!(svc.post("livery", "x").map(drop), Err(expected.clone()));
            assert_eq!(svc.list("livery", None).map(drop), Err(expected));
            // Only a 401 on a write forgets the session.
            assert!(store.has_token());
        }
    }

    #[test]
    fn a_token_is_never_part_of_an_error_text() {
        let (api, store, avatars) = setup();
        *api.write.lock().unwrap() = Some(Err(CommentsApiError::Unreachable("down".into())));
        let error = service(&api, &store, &avatars)
            .post("livery", "x")
            .unwrap_err();
        assert!(!format!("{error:?} {error}").contains(TOKEN));
    }

    #[test]
    fn deleting_calls_the_server_with_the_ids() {
        let (api, store, avatars) = setup();
        service(&api, &store, &avatars)
            .delete("livery", "c9")
            .unwrap();
        assert_eq!(api.calls.lock().unwrap()[0].0, "delete:c9");
    }

    #[test]
    fn avatars_are_fetched_once_per_url() {
        let (api, store, avatars) = setup();
        *api.page.lock().unwrap() = Some(Ok(ApiCommentsPage {
            comments: vec![
                comment("1", Some("https://cdn.example/a.png")),
                comment("2", Some("https://cdn.example/a.png")),
                comment("3", None),
            ],
            next_cursor: None,
        }));
        let svc = service(&api, &store, &avatars);
        let page = svc.list("livery", None).unwrap();
        assert_eq!(
            page.comments[0].author_avatar_data_url,
            page.comments[1].author_avatar_data_url
        );
        assert!(page.comments[2].author_avatar_data_url.is_none());
        *api.page.lock().unwrap() = Some(Ok(ApiCommentsPage {
            comments: vec![comment("4", Some("https://cdn.example/a.png"))],
            next_cursor: None,
        }));
        svc.list("livery", None).unwrap();
        assert_eq!(avatars.fetched.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_failed_avatar_download_leaves_no_avatar_and_is_not_retried() {
        let (api, store, _) = setup();
        let avatars = Arc::new(CountingAvatars {
            fail: true,
            ..CountingAvatars::default()
        });
        let svc = service(&api, &store, &avatars);
        for _ in 0..2 {
            *api.page.lock().unwrap() = Some(Ok(ApiCommentsPage {
                comments: vec![comment("1", Some("https://cdn.example/a.png"))],
                next_cursor: None,
            }));
            let page = svc.list("livery", None).unwrap();
            assert!(page.comments[0].author_avatar_data_url.is_none());
        }
        assert_eq!(avatars.fetched.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_avatar_cache_is_bounded() {
        let mut cache = AvatarCache::default();
        for i in 0..(AVATAR_CACHE_CAPACITY + 10) {
            cache.insert(format!("https://cdn.example/{i}.png"), None);
        }
        assert_eq!(cache.entries.len(), AVATAR_CACHE_CAPACITY);
        assert_eq!(cache.order.len(), AVATAR_CACHE_CAPACITY);
        assert!(!cache.entries.contains_key("https://cdn.example/0.png"));
        assert!(cache.entries.contains_key(&format!(
            "https://cdn.example/{}.png",
            AVATAR_CACHE_CAPACITY + 9
        )));
    }
}
