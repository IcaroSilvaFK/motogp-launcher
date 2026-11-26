//! Community server application layer: ports and the shared error type.
//! No IO and no database types here (so SQLite can be replaced later).

pub mod auth;
pub mod error;
pub mod ports;
pub mod stars;

pub use auth::AuthService;
pub use error::AppError;
pub use stars::{ModStats, StarRepository, StarService};
