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

pub use data_dir::DataDir;
pub use game_files::FsGameFiles;
pub use steam::{parse_app_manifest, parse_library_folders, SteamLibraries};
pub use profile_store::{JsonProfileStore, SystemClock};
pub use store::{load_json, save_json, JsonConfigStore};
#[cfg(windows)]
pub use windows_registry::read_steam_path;
