//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod bootstrap;
mod error;
mod error_game;
mod error_library;
mod error_profiles;

pub use bootstrap::run;
pub use error::CommandError;
