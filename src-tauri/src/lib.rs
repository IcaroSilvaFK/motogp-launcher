//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod adapters;
mod bootstrap;
mod catalog_fallback;
mod commands;
mod compose;
mod dto;
mod error;
mod error_catalog;
mod error_game;
mod error_library;
mod error_play;
mod error_profiles;
mod startup;

pub use adapters::{
    is_steam_url, TauriEvents, TauriFolderOpener, TauriOpener, TauriWebOpener, SESSION_ERROR_EVENT,
    SESSION_EVENT,
};
pub use bootstrap::run;
pub use catalog_fallback::UnavailableCatalogSource;
pub use commands::configure;
pub use compose::{compose, AppState, Catalog, Profiles, Settings, Storage};
pub use dto::{
    CatalogDownloadDto, CatalogDto, CatalogEntryDto, ConfigDto, CurrentSessionDto,
    DeploySessionDto, GameInfoDto, ModDto, ProfileDto, RecoveryStatusDto, SessionErrorDto,
    SessionEventDto, StorageCategoryDto, StorageUsageDto,
};
pub use error::CommandError;
pub use startup::run_recovery;
