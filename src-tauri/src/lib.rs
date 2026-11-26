//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod adapters;
mod ads_fallback;
mod bootstrap;
mod catalog_fallback;
mod catalog_install_support;
mod commands;
mod commands_install;
mod commands_stars;
mod community_config;
mod community_fallback;
mod community_stars_fallback;
mod compose;
mod dto;
mod dto_stars;
mod error;
mod error_ads;
mod error_catalog;
mod error_community;
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
pub use community_config::COMMUNITY_SERVER_URL;
pub use community_fallback::{MemoryCredentialStore, UnavailableCommunityApi};
pub use community_stars_fallback::UnavailableModStatsApi;
pub use compose::{
    compose, compose_with_downloader, Ads, AppState, Catalog, Installer, Profiles, Settings,
    Storage,
};
pub use dto::{
    AdDto, CatalogDownloadDto, CatalogDto, CatalogEntryDto, CommunityProfileDto,
    CommunityStatusDto, ConfigDto, CurrentSessionDto, DeploySessionDto, GameInfoDto, ModDto,
    ProfileDto, RecoveryStatusDto, SessionErrorDto, SessionEventDto, StorageCategoryDto,
    StorageUsageDto,
};
pub use dto_stars::{ModStatsDto, ModStatsListDto};
pub use error::CommandError;
pub use startup::run_recovery;
