//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod adapters;
mod ads_fallback;
mod app_log;
mod bootstrap;
mod catalog_fallback;
mod catalog_install_support;
mod commands;
mod commands_comments;
mod commands_diagnostics;
mod commands_downloads;
mod commands_install;
mod commands_publish;
mod commands_stars;
mod comments_fallback;
mod community_config;
mod community_fallback;
mod community_publish_fallback;
mod community_stars_fallback;
mod compose;
mod download_stats_fallback;
mod dto;
mod dto_comments;
mod dto_publish;
mod dto_stars;
mod error;
mod error_ads;
mod error_catalog;
mod error_comments;
mod error_community;
mod error_game;
mod error_install;
mod error_library;
mod error_play;
mod error_profiles;
mod error_publish;
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
pub use commands_publish::COMMUNITY_PUBLISH_PROGRESS_EVENT;
pub use community_config::COMMUNITY_SERVER_URL;
pub use community_fallback::{MemoryCredentialStore, UnavailableCommunityApi};
pub use community_publish_fallback::UnavailablePublishApi;
pub use community_stars_fallback::UnavailableModStatsApi;
pub use compose::{
    compose, compose_with_downloader, Ads, AppState, Catalog, Diagnostics, Installer, Profiles,
    Settings, Storage,
};
pub use download_stats_fallback::UnavailableDownloadStats;
pub use dto::{
    AdDto, CatalogDownloadDto, CatalogDto, CatalogEntryDto, CommunityProfileDto,
    CommunityStatusDto, ConfigDto, CurrentSessionDto, DeploySessionDto, GameInfoDto, ModDto,
    ProfileDto, RecoveryStatusDto, SessionErrorDto, SessionEventDto, StorageCategoryDto,
    StorageUsageDto,
};
pub use dto_publish::{ModVersionDto, MyModDto, PublishProgressDto, PublishRequestDto};
pub use dto_stars::{ModStatsDto, ModStatsListDto};
pub use error::CommandError;
pub use startup::run_recovery;
