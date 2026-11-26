//! Infrastructure layer: adapters that implement the application ports.
//!
//! Fs/JSON, ZIP/SHA-256 and Steam/process code goes here when needed, never
//! in the core (`domain`/`application`).

mod ad_cache;
mod ad_http;
mod avatar_http;
mod bundled_catalog;
mod catalog_cache;
mod catalog_http;
mod community_comments_http;
mod community_http;
mod community_stars_http;
mod credential_store;
mod data_dir;
mod deploy_files;
mod deployed_repository;
mod download_http;
mod download_staging;
mod game_files;
mod instance_lock;
mod library_files;
mod library_repository;
mod loopback;
mod mod_library;
mod pkce;
mod processes;
mod profile_store;
mod session_runtime;
mod sha256_hasher;
mod steam;
mod steam_artwork;
mod storage_scanner;
mod store;
mod system_clock;
mod win_path;
#[cfg(windows)]
mod windows_registry;
mod zip_archive;

pub use ad_cache::JsonAdCache;
pub use ad_http::{
    parse_ads, FeedEntry, HttpAdBeaconSink, HttpAdConfig, HttpAdSource, DEFAULT_ADS_URL,
    DEFAULT_RAW_BASE_URL,
};
pub use avatar_http::{HttpAvatarFetcher, MAX_AVATAR_BYTES};
pub use bundled_catalog::bundled_catalog;
pub use catalog_cache::JsonCatalogCache;
pub use catalog_http::{parse_catalog, HttpCatalogConfig, HttpCatalogSource, DEFAULT_CATALOG_URL};
pub use community_http::{HttpCommunityApi, HttpCommunityConfig};
pub use community_stars_http::HttpModStatsApi;
pub use credential_store::{WindowsCredentialStore, COMMUNITY_TARGET};
pub use data_dir::DataDir;
pub use deploy_files::{FsDeployFiles, LinkFn};
pub use deployed_repository::JsonDeployedRepository;
pub use download_http::{HttpDownloadConfig, HttpModDownloader};
pub use download_staging::FsDownloadStaging;
pub use game_files::FsGameFiles;
pub use instance_lock::FileInstanceLock;
pub use library_files::FsLibraryFiles;
pub use library_repository::JsonModRepository;
pub use loopback::TcpLoopbackReceiver;
pub use mod_library::{mod_library, ModLibrary};
pub use pkce::OsPkce;
pub use processes::{any_inside_root, is_inside_root, SysinfoProcesses};
pub use profile_store::{JsonProfileStore, SystemClock};
pub use session_runtime::{AtomicSessionLock, ThreadRunner};
pub use sha256_hasher::Sha256Hasher;
pub use steam::{parse_app_manifest, parse_library_folders, SteamLibraries};
pub use steam_artwork::SteamArtwork;
pub use storage_scanner::FsStorageScanner;
pub use store::{load_json, save_json, JsonConfigStore};
pub use system_clock::MonotonicClock;
pub use win_path::normalize_windows_path;
#[cfg(windows)]
pub use windows_registry::read_steam_path;
pub use zip_archive::{ZipArchiveReader, ZipLimits};
