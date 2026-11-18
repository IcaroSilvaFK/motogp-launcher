//! Application layer: use cases and ports (traits).
//!
//! Depends only on `motogp-domain`. Ports and use cases arrive in the issues
//! that need them.

mod config_service;
mod error;
mod ports;

pub use config_service::ConfigService;
pub use error::AppError;
pub use ports::ConfigRepository;
