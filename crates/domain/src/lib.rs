//! Domain layer: entities, value objects and pure rules.
//!
//! No I/O, no serde, no Tauri and no other project crates.
//! Entities arrive in the next issues, always driven by tests.

mod config;
mod game;
mod profile;

pub use config::Config;
pub use game::{
    find_app_id, install_dir_matches, is_engine_folder, is_motogp_folder, mods_dir_for, GameInfo,
    SteamManifest,
};
pub use profile::{normalize_name, Profile, ProfileError, VANILLA_ID};
