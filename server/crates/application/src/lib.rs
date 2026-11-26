//! Community server application layer: ports and the shared error type.
//! No IO and no database types here (so SQLite can be replaced later).

pub mod auth;
pub mod comments;
pub mod downloads;
pub mod error;
pub mod mod_ports;
pub mod mods;
pub mod ports;
pub mod stars;

pub use auth::AuthService;
pub use downloads::DownloadService;
pub use error::AppError;
pub use mods::ModHostingService;
pub use stars::{ModStats, StarRepository, StarService};
