//! Interface layer (Tauri): bootstrap, thin commands, DTO conversion and error
//! mapping. The adapters are composed here.

mod bootstrap;
mod error;

pub use bootstrap::run;
pub use error::CommandError;
