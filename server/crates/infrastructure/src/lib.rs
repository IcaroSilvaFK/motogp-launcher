//! Community server infrastructure: configuration, SQLite and system adapters.

pub mod config;
pub mod dev_discord;
pub mod discord;
pub mod sqlite;
mod sqlite_comments;
pub mod sqlite_downloads;
mod sqlite_moderation;
pub mod sqlite_mods;
pub mod sqlite_stars;
pub mod storage;
pub mod system;

pub use config::{Config, ConfigError, DevFakeAuth};
pub use dev_discord::DevFakeDiscord;
pub use discord::DiscordHttp;
pub use sqlite::SqliteStore;
pub use storage::LocalModStorage;
pub use system::{OsTokenGenerator, Sha256Hasher, SystemClock};
