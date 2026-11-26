//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod adapters;
mod ads_fallback;
mod bootstrap;
mod catalog_fallback;
mod catalog_install_support;
mod commands;
mod commands_install;
mod compose;
mod dto;
mod error;
mod error_ads;
mod error_catalog;
mod error_game;
mod error_install;
mod error_library;
mod error_play;
mod error_profiles;
mod error_support;
mod startup;

pub use adapters::{
    is_steam_url, TauriEvents, TauriFolderOpener, TauriOpener, TauriWebOpener, WebPolicy,
    SESSION_ERROR_EVENT, SESSION_EVENT,
};
pub use bootstrap::run;
pub use catalog_fallback::UnavailableCatalogSource;
pub use catalog_install_support::{production_downloader, UnavailableModDownloader};
pub use commands::configure;
pub use commands_install::{CatalogInstallProgressDto, CATALOG_INSTALL_PROGRESS_EVENT};
pub use compose::{
    compose, compose_with_downloader, Ads, AppState, Catalog, Installer, Profiles, Settings,
    Storage,
};
pub use dto::{
    AdDto, CatalogDownloadDto, CatalogDto, CatalogEntryDto, ConfigDto, CurrentSessionDto,
    DeploySessionDto, GameInfoDto, ModDto, ProfileDto, RecoveryStatusDto, SessionErrorDto,
    SessionEventDto, StorageCategoryDto, StorageUsageDto,
};
pub use error::CommandError;
pub use startup::run_recovery;
