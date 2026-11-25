//! Wiring helpers of the catalog install (#74): the production downloader and
//! the fallback used when its HTTP client cannot be built.
use motogp_application::{
    CancelFlag, DownloadError, DownloadProgress, DownloadRequest, ModDownloader,
};
use motogp_infrastructure::{HttpDownloadConfig, HttpModDownloader};
use std::path::Path;
use std::sync::Arc;

/// Downloader that always fails, so the app still starts when the HTTPS
/// client cannot be built (mirrors `UnavailableCatalogSource`).
#[derive(Debug, Clone)]
pub struct UnavailableModDownloader {
    reason: String,
}

impl UnavailableModDownloader {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl ModDownloader for UnavailableModDownloader {
    fn download(
        &self,
        _request: &DownloadRequest,
        _dest: &Path,
        _progress: &mut dyn FnMut(DownloadProgress),
        _cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        Err(DownloadError::Network(self.reason.clone()))
    }
}

/// The HTTPS (production) downloader, or the unavailable fallback.
pub fn production_downloader() -> Arc<dyn ModDownloader> {
    match HttpModDownloader::new(HttpDownloadConfig::production(env!("CARGO_PKG_VERSION"))) {
        Ok(downloader) => Arc::new(downloader),
        Err(error) => {
            eprintln!("mod downloader unavailable: {error}");
            Arc::new(UnavailableModDownloader::new(error.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unavailable_downloader_always_fails_with_its_reason() {
        let downloader = UnavailableModDownloader::new("no tls");
        let request = DownloadRequest {
            url: "https://example.com/a.zip".into(),
            max_bytes: 1,
        };
        let result = downloader.download(&request, Path::new("a"), &mut |_| {}, &CancelFlag::new());
        assert_eq!(result, Err(DownloadError::Network("no tls".into())));
    }

    #[test]
    fn the_production_downloader_builds() {
        let downloader = production_downloader();
        // https only: a plain http URL never reaches the network.
        let request = DownloadRequest {
            url: "http://example.com/a.zip".into(),
            max_bytes: 1,
        };
        let result = downloader.download(&request, Path::new("a"), &mut |_| {}, &CancelFlag::new());
        assert!(matches!(result, Err(DownloadError::Network(_))));
    }
}
