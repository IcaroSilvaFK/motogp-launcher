//! Fallback adapter of publishing. When the HTTP client cannot be built (see
//! `bootstrap.rs`) the app still starts: the community is simply offline. Also
//! the default of a freshly composed state.
use motogp_application::{
    CreateUpload, ModPublishApi, ModVersion, MyMod, PublishApiError, UploadSession, UploadStatus,
};
use motogp_domain::SessionToken;

/// A server that can never be reached.
pub struct UnavailablePublishApi {
    reason: String,
}

impl UnavailablePublishApi {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn error(&self) -> PublishApiError {
        PublishApiError::Network(self.reason.clone())
    }
}

impl ModPublishApi for UnavailablePublishApi {
    fn create_upload(
        &self,
        _token: &SessionToken,
        _upload: &CreateUpload,
    ) -> Result<UploadSession, PublishApiError> {
        Err(self.error())
    }

    fn put_chunk(
        &self,
        _token: &SessionToken,
        _upload_id: &str,
        _offset: u64,
        _bytes: &[u8],
    ) -> Result<u64, PublishApiError> {
        Err(self.error())
    }

    fn upload_status(
        &self,
        _token: &SessionToken,
        _upload_id: &str,
    ) -> Result<UploadStatus, PublishApiError> {
        Err(self.error())
    }

    fn add_screenshot(
        &self,
        _token: &SessionToken,
        _upload_id: &str,
        _content_type: &str,
        _bytes: &[u8],
    ) -> Result<(), PublishApiError> {
        Err(self.error())
    }

    fn complete(
        &self,
        _token: &SessionToken,
        _upload_id: &str,
    ) -> Result<ModVersion, PublishApiError> {
        Err(self.error())
    }

    fn abort(&self, _token: &SessionToken, _upload_id: &str) -> Result<(), PublishApiError> {
        Err(self.error())
    }

    fn my_mods(&self, _token: &SessionToken) -> Result<Vec<MyMod>, PublishApiError> {
        Err(self.error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unavailable_server_is_always_a_network_error() {
        let api = UnavailablePublishApi::new("no tls");
        let token = SessionToken::new("t");
        let upload = CreateUpload {
            mod_id: "a".into(),
            title: "t".into(),
            description: "d".into(),
            long_description: None,
            category: "c".into(),
            game: "g".into(),
            version: "1".into(),
            file_name: "a.zip".into(),
            size: 1,
            sha256: "h".into(),
            ownership_confirmed: true,
        };
        assert!(matches!(
            api.create_upload(&token, &upload),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.put_chunk(&token, "u", 0, &[1]),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.upload_status(&token, "u"),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.add_screenshot(&token, "u", "image/png", &[1]),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.complete(&token, "u"),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.abort(&token, "u"),
            Err(PublishApiError::Network(_))
        ));
        assert!(matches!(
            api.my_mods(&token),
            Err(PublishApiError::Network(_))
        ));
    }
}
