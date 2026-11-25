//! Tauri adapters of the application ports: event publishing and the Steam
//! opener. They are the only code that touches `AppHandle`.
use crate::dto::{SessionErrorDto, SessionEventDto};
use motogp_application::{
    AppError, FolderOpener, SessionEvent, SessionPublisher, UrlOpener, WebPageOpener,
};
use motogp_domain::check_page_url;
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
            eprintln!("could not emit the session event: {error}");
        }
    }

    fn report_error(&self, message: &str) {
        let payload = SessionErrorDto {
            message: message.to_string(),
        };
        if let Err(error) = self.handle.emit(SESSION_ERROR_EVENT, payload) {
            eprintln!("could not emit the session error: {error}");
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

/// `WebPageOpener` backed by the Tauri opener plugin. It opens a web page from
/// Rust, so the frontend capability stays limited to `steam://` URLs. The
/// page policy is checked again here, defensively, before anything is opened.
pub struct TauriWebOpener<R: Runtime> {
    handle: AppHandle<R>,
}

impl<R: Runtime> TauriWebOpener<R> {
    pub fn new(handle: AppHandle<R>) -> Self {
        Self { handle }
    }
}

impl<R: Runtime> WebPageOpener for TauriWebOpener<R> {
    fn open_page(&self, url: &str) -> Result<(), AppError> {
        check_page_url(url).map_err(|error| AppError::Read {
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
