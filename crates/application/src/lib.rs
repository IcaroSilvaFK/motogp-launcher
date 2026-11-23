//! Application layer: use cases and ports (traits).
//!
//! Depends only on `motogp-domain`. Ports and use cases arrive in the issues
//! that need them.

mod config_service;
mod deploy_error;
mod deploy_ports;
mod deploy_service;
mod deploy_session;
mod error;
mod game_error;
mod game_ports;
mod game_service;
mod game_settings;
mod library_commands;
mod library_error;
mod library_ports;
mod mod_service;
mod play_error;
mod play_ports;
mod play_service;
mod ports;
mod profile_mods;
mod profile_ports;
mod profile_service;
mod session_error;
mod session_ports;
mod session_service;
mod storage_ports;
mod storage_service;

pub use config_service::ConfigService;
pub use deploy_error::DeployError;
pub use deploy_ports::{DeployFiles, DeployedRepository, DirCheck};
pub use deploy_service::{DeployMod, DeployService};
pub use deploy_session::DeploySession;
pub use error::AppError;
pub use game_error::GameError;
pub use game_ports::{GameFiles, SteamLocator};
pub use game_service::GameService;
pub use game_settings::GameSettings;
pub use library_commands::{import_mods, remove_mod, ImportBatchError, RemoveModError};
pub use library_error::{ArchiveLimit, LibraryError};
pub use library_ports::{ArchiveReader, ContentHasher, LibraryFiles, ModRepository};
pub use mod_service::ModService;
pub use play_error::PlayError;
pub use play_ports::{
    BackgroundRunner, Deployer, InstanceGuard, InstanceLock, ModPlanner, PlayGame, RecoveryState,
    RecoveryStatus, SessionEvent, SessionLock, SessionPublisher, SessionRuntime, SessionState,
    SpawnError,
};
pub use play_service::PlayService;
pub use ports::ConfigRepository;
pub use profile_mods::{list_conflicts, resolve_profile_mods, ProfileModPlanner};
pub use profile_ports::{Clock, ProfileRepository};
pub use profile_service::{ProfileService, ProfileServiceError};
pub use session_error::SessionError;
pub use session_ports::{ProcessInspector, SessionClock, TrackOutcome, UrlOpener, POLL};
pub use session_service::SessionService;
pub use storage_ports::{FolderOpener, ScannedEntry, StorageScanner};
pub use storage_service::StorageService;
