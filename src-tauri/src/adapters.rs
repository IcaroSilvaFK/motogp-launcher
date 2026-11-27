//! Tauri adapters of the application ports: event publishing and the Steam
//! opener. They are the only code that touches `AppHandle`.
use crate::dto::{SessionErrorDto, SessionEventDto};
use motogp_application::{
    AppError, FolderOpener, SessionEvent, SessionPublisher, UrlOpener, WebPageOpener,
};
use motogp_domain::{check_https_url, check_page_url, PageUrlError};
use std::path::Path;
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_opener::OpenerExt;

/// Event name of session state changes.
pub const SESSION_EVENT: &str = "session";
/// Event name of failures that must be visible (never a false success).
pub const SESSION_ERROR_EVENT: &str = "session-error";

/// `SessionPublisher` that emits Tauri events.
pub struct TauriEvents<R: Runtime> {
    handle: AppHandle<R>,
}

impl<R: Runtime> TauriEvents<R> {
    pub fn new(handle: AppHandle<R>) -> Self {
        Self { handle }
    }
}

impl<R: Runtime> SessionPublisher for TauriEvents<R> {
    fn publish(&self, event: SessionEvent) {
        if let Err(error) = self
            .handle
            .emit(SESSION_EVENT, SessionEventDto::from(event))
        {
            crate::app_log::warn(&format!("could not emit the session event: {error}"));
        }
    }

    fn report_error(&self, message: &str) {
        let payload = SessionErrorDto {
            message: message.to_string(),
        };
        if let Err(error) = self.handle.emit(SESSION_ERROR_EVENT, payload) {
            crate::app_log::warn(&format!("could not emit the session error: {error}"));
        }
    }
}

/// Only the Steam protocol may be opened by the launcher.
pub fn is_steam_url(url: &str) -> bool {
    url.starts_with("steam://")
}

/// `UrlOpener` backed by the Tauri opener plugin, restricted to `steam://`
/// both here and in the capability scope.
pub struct TauriOpener<R: Runtime> {
    handle: AppHandle<R>,
}

impl<R: Runtime> TauriOpener<R> {
    pub fn new(handle: AppHandle<R>) -> Self {
        Self { handle }
    }
}

impl<R: Runtime> UrlOpener for TauriOpener<R> {
    fn open(&self, url: &str) -> Result<(), AppError> {
        if !is_steam_url(url) {
            return Err(AppError::Read {
                location: url.to_string(),
                detail: "only steam:// URLs can be opened".to_string(),
            });
        }
        self.handle
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|error| AppError::Read {
                location: url.to_string(),
                detail: error.to_string(),
            })
    }
}

/// `FolderOpener` backed by the Tauri opener plugin. It opens a path from
/// Rust, so the frontend capability stays limited to `steam://` URLs.
pub struct TauriFolderOpener<R: Runtime> {
    handle: AppHandle<R>,
}

impl<R: Runtime> TauriFolderOpener<R> {
    pub fn new(handle: AppHandle<R>) -> Self {
        Self { handle }
    }
}

impl<R: Runtime> FolderOpener for TauriFolderOpener<R> {
    fn open_folder(&self, path: &Path) -> Result<(), AppError> {
        self.handle
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|error| AppError::Read {
                location: path.display().to_string(),
                detail: error.to_string(),
            })
    }
}

/// Which web pages a [`TauriWebOpener`] may open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebPolicy {
    /// Only the catalog and support hosts (`check_page_url`).
    AllowListedHosts,
    /// Any well-formed `https://` URL (sponsor click URLs, which come from the
    /// validated ad feed and are never supplied by the WebView).
    AnyHttps,
    /// Only the community sign-in start page on the configured server (issue #122).
    CommunityStart,
}

impl WebPolicy {
    pub fn check(self, url: &str) -> Result<(), PageUrlError> {
        match self {
            WebPolicy::AllowListedHosts => check_page_url(url),
            WebPolicy::AnyHttps => check_https_url(url),
            WebPolicy::CommunityStart => check_community_start(url),
        }
    }
}

/// The community sign-in start page is the only URL this policy opens.
fn check_community_start(url: &str) -> Result<(), PageUrlError> {
    if crate::community_config::is_community_start_url(url) {
        Ok(())
    } else {
        Err(PageUrlError::NotHttps)
    }
}

/// `WebPageOpener` backed by the Tauri opener plugin. It opens a web page from
/// Rust, so the frontend capability stays limited to `steam://` URLs. The
/// page policy is checked again here, defensively, before anything is opened.
pub struct TauriWebOpener<R: Runtime> {
    handle: AppHandle<R>,
    policy: WebPolicy,
}

impl<R: Runtime> TauriWebOpener<R> {
    /// Catalog and support pages: allow-listed hosts only.
    pub fn new(handle: AppHandle<R>) -> Self {
        Self {
            handle,
            policy: WebPolicy::AllowListedHosts,
        }
    }

    /// Sponsor pages: any plain `https://` URL on any host.
    pub fn https_only(handle: AppHandle<R>) -> Self {
        Self {
            handle,
            policy: WebPolicy::AnyHttps,
        }
    }

    /// Community sign-in: only the configured server's start page.
    pub fn community(handle: AppHandle<R>) -> Self {
        Self {
            handle,
            policy: WebPolicy::CommunityStart,
        }
    }
}

impl<R: Runtime> WebPageOpener for TauriWebOpener<R> {
    fn open_page(&self, url: &str) -> Result<(), AppError> {
        self.policy.check(url).map_err(|error| AppError::Read {
            location: url.to_string(),
            detail: error.to_string(),
        })?;
        self.handle
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|error| AppError::Read {
                location: url.to_string(),
                detail: error.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_community_policy_only_opens_the_sign_in_start_page() {
        let start = format!(
            "{}/auth/discord/start?redirect=x",
            crate::community_config::COMMUNITY_SERVER_URL
        );
        assert_eq!(WebPolicy::CommunityStart.check(&start), Ok(()));
        for url in [
            "https://sponsor.example/x",
            "https://overtake.gg/x",
            "javascript:alert(1)",
        ] {
            assert!(WebPolicy::CommunityStart.check(url).is_err(), "{url}");
        }
    }
    #[test]
    fn the_https_only_policy_accepts_any_plain_https_url() {
        assert_eq!(
            WebPolicy::AnyHttps.check("https://sponsor.example/x"),
            Ok(())
        );
    }

    #[test]
    fn the_https_only_policy_rejects_unsafe_urls() {
        for url in [
            "http://sponsor.example/x",
            "javascript:alert(1)",
            "https://user:pw@sponsor.example/x",
            "https://sponsor.example:8443/x",
        ] {
            assert!(WebPolicy::AnyHttps.check(url).is_err(), "{url}");
        }
    }

    #[test]
    fn the_default_policy_still_requires_an_allow_listed_host() {
        assert_eq!(
            WebPolicy::AllowListedHosts.check("https://overtake.gg/x"),
            Ok(())
        );
        assert!(WebPolicy::AllowListedHosts
            .check("https://sponsor.example/x")
            .is_err());
    }
}
