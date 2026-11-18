//! Domain layer: entities, value objects and pure rules.
//!
//! No I/O, no serde, no Tauri and no other project crates.
//! Entities arrive in the next issues, always driven by tests.

mod config;
mod profile;

pub use config::Config;
pub use profile::{normalize_name, Profile, ProfileError, VANILLA_ID};
