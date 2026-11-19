//! Domain layer: entities, value objects and pure rules.
//!
//! No I/O, no serde, no Tauri and no other project crates.
//! Entities arrive in the next issues, always driven by tests.

mod config;
mod deploy;
mod game;
mod mod_entry;
mod mod_grouping;
mod profile;
mod session;

pub use config::Config;
pub use deploy::{deployed_name, is_mods_dir, is_safe_file_name, Deployed, DeployedFile};
pub use game::{
    find_app_id, install_dir_matches, is_engine_folder, is_motogp_folder, mods_dir_for, GameInfo,
    SteamManifest,
};
pub use mod_entry::{is_valid_mod_id, mod_id_from_hash, ModEntry, MOD_ID_LEN};
pub use mod_grouping::{
    base_name, display_name, extension_of, group_pak_files, is_mod_file, GroupingError, ModGroup,
    MOD_EXTENSIONS,
};
pub use profile::{normalize_name, Profile, ProfileError, VANILLA_ID};
pub use session::{
    launch_url, watch_step, LaunchError, WatchEvent, WatchState, MISSING_POLLS_TO_END,
    NEVER_STARTED_AFTER,
};
