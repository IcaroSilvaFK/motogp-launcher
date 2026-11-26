//! Fallback of the mod comments (#118): when the production adapters cannot be
//! built the app still starts and the comments are simply offline.
use crate::{MemoryCredentialStore, UnavailableCommunityApi};
use motogp_application::{
    ApiComment, ApiCommentsPage, AvatarFetcher, CommentsApiError, CommunityCommentsApi,
    CommunityCommentsService,
};
use motogp_domain::SessionToken;
use std::sync::Arc;

impl UnavailableCommunityApi {
    fn comments_error(&self) -> CommentsApiError {
        CommentsApiError::Unreachable(self.reason.clone())
    }
}

impl CommunityCommentsApi for UnavailableCommunityApi {
    fn list(
        &self,
        _mod_id: &str,
        _cursor: Option<&str>,
        _token: Option<&SessionToken>,
    ) -> Result<ApiCommentsPage, CommentsApiError> {
        Err(self.comments_error())
    }

    fn post(
        &self,
        _mod_id: &str,
        _body: &str,
        _token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        Err(self.comments_error())
    }

    fn edit(
        &self,
        _mod_id: &str,
        _comment_id: &str,
        _body: &str,
        _token: &SessionToken,
    ) -> Result<ApiComment, CommentsApiError> {
        Err(self.comments_error())
    }

    fn delete(
        &self,
        _mod_id: &str,
        _comment_id: &str,
        _token: &SessionToken,
    ) -> Result<(), CommentsApiError> {
        Err(self.comments_error())
    }
}

/// Initials only.
struct NoAvatars;

impl AvatarFetcher for NoAvatars {
    fn fetch(&self, _url: &str) -> Option<String> {
        None
    }
}

/// A comments service that can never reach the server: the default of
/// `compose`, replaced in production with `AppState::with_comments`.
pub(crate) fn offline_comments() -> Arc<CommunityCommentsService> {
    Arc::new(CommunityCommentsService::new(
        Arc::new(UnavailableCommunityApi::new("comments not configured")),
        Arc::new(MemoryCredentialStore::default()),
        Arc::new(NoAvatars),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::CommentsError;

    #[test]
    fn the_unavailable_server_is_unreachable_for_every_comment_call() {
        let api = UnavailableCommunityApi::new("no tls");
        let token = SessionToken::new("t");
        assert!(matches!(
            api.list("m", None, None),
            Err(CommentsApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.post("m", "b", &token),
            Err(CommentsApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.edit("m", "c", "b", &token),
            Err(CommentsApiError::Unreachable(_))
        ));
        assert!(matches!(
            api.delete("m", "c", &token),
            Err(CommentsApiError::Unreachable(_))
        ));
    }

    #[test]
    fn the_offline_default_reads_offline_and_writes_signed_out() {
        let service = offline_comments();
        assert_eq!(
            service.list("livery", None).map(drop),
            Err(CommentsError::Offline)
        );
        assert_eq!(
            service.post("livery", "x").map(drop),
            Err(CommentsError::SignedOut)
        );
    }
}
