//! Infrastructure layer: adapters that implement the application ports.
//!
//! Fs/JSON, ZIP/SHA-256 and Steam/process code goes here when needed, never
//! in the core (`domain`/`application`).

mod data_dir;
mod game_files;
mod profile_store;
mod steam;
mod store;
#[cfg(windows)]
mod windows_registry;
mod library_files;
mod library_repository;
mod mod_library;
mod sha256_hasher;
mod zip_archive;

pub use data_dir::DataDir;
pub use game_files::FsGameFiles;
pub use profile_store::{JsonProfileStore, SystemClock};
pub use steam::{parse_app_manifest, parse_library_folders, SteamLibraries};
pub use store::{load_json, save_json, JsonConfigStore};
#[cfg(windows)]
pub use windows_registry::read_steam_path;
pub use library_files::FsLibraryFiles;
pub use library_repository::JsonModRepository;
pub use mod_library::{mod_library, ModLibrary};
pub use sha256_hasher::Sha256Hasher;
pub use zip_archive::{ZipArchiveReader, ZipLimits};
