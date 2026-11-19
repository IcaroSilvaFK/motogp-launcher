//! Infrastructure layer: adapters that implement the application ports.
//!
//! Fs/JSON, ZIP/SHA-256 and Steam/process code goes here when needed, never
//! in the core (`domain`/`application`).

mod data_dir;
mod profile_store;
mod store;

pub use data_dir::DataDir;
pub use profile_store::{JsonProfileStore, SystemClock};
pub use store::{load_json, save_json, JsonConfigStore};
