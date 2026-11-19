//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod adapters;
mod bootstrap;
mod commands;
mod compose;
mod dto;
mod error;
mod error_game;
mod error_library;
mod error_play;
mod error_profiles;
mod startup;

pub use adapters::{is_steam_url, TauriEvents, TauriOpener, SESSION_ERROR_EVENT, SESSION_EVENT};
pub use bootstrap::run;
pub use commands::configure;
pub use compose::{compose, AppState, Profiles, Settings};
pub use dto::{
    ConfigDto, GameInfoDto, ModDto, ProfileDto, SessionErrorDto, SessionEventDto,
};
pub use error::CommandError;
pub use startup::run_recovery;
