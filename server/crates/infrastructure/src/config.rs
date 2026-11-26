//! Environment configuration. Errors name the variable, never its value.

use std::fmt;
use std::net::SocketAddr;

use community_domain::DiscordId;

pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8787";
pub const DEFAULT_DISCORD_API_BASE: &str = "https://discord.com/api/v10";
pub const DEFAULT_MOD_MAX_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const DEFAULT_MOD_USER_QUOTA_BYTES: u64 = 10 * 1024 * 1024 * 1024;
pub const DEFAULT_MOD_UPLOADS_PER_DAY: u32 = 10;

#[derive(Clone)]
pub struct Config {
    pub discord_client_id: String,
    pub discord_client_secret: String,
    /// Public HTTPS origin, without a trailing slash.
    pub public_base_url: String,
    pub database_url: String,
    pub admin_discord_ids: Vec<DiscordId>,
    pub bind_addr: SocketAddr,
    pub trust_proxy_headers: bool,
    pub discord_api_base: String,
    /// Root of the mod files (`incoming/`, `pending/`, `public/`).
    pub mod_storage_dir: std::path::PathBuf,
    pub mod_max_upload_bytes: u64,
    pub mod_user_quota_bytes: u64,
    pub mod_uploads_per_day: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Missing(&'static str),
    Invalid(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing required environment variable {name}"),
            Self::Invalid(name) => write!(f, "invalid value for environment variable {name}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Reads the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Builds the config from any variable lookup (tests pass a map).
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let required = |name: &'static str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::Missing(name))
        };
        let optional = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };

        let discord_client_id = required("DISCORD_CLIENT_ID")?;
        let discord_client_secret = required("DISCORD_CLIENT_SECRET")?;
        let public_base_url = required("PUBLIC_BASE_URL")?
            .trim_end_matches('/')
            .to_owned();
        let database_url = required("DATABASE_URL")?;
        let admin_ids_raw = required("ADMIN_DISCORD_IDS")?;

        let admin_discord_ids = admin_ids_raw
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| DiscordId::parse(part).ok_or(ConfigError::Invalid("ADMIN_DISCORD_IDS")))
            .collect::<Result<Vec<_>, _>>()?;

        let bind_addr = optional("BIND_ADDR")
            .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_owned())
            .parse()
            .map_err(|_| ConfigError::Invalid("BIND_ADDR"))?;

        let trust_proxy_headers = match optional("TRUST_PROXY_HEADERS").as_deref() {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => return Err(ConfigError::Invalid("TRUST_PROXY_HEADERS")),
        };

        let discord_api_base = optional("DISCORD_API_BASE")
            .unwrap_or_else(|| DEFAULT_DISCORD_API_BASE.to_owned())
            .trim_end_matches('/')
            .to_owned();

        let mod_storage_dir = required("MOD_STORAGE_DIR")?.into();
        let positive = |name: &'static str, default: u64| -> Result<u64, ConfigError> {
            match optional(name) {
                None => Ok(default),
                Some(value) => value
                    .parse::<u64>()
                    .ok()
                    .filter(|number| *number > 0)
                    .ok_or(ConfigError::Invalid(name)),
            }
        };
        let mod_max_upload_bytes = positive("MOD_MAX_UPLOAD_BYTES", DEFAULT_MOD_MAX_UPLOAD_BYTES)?;
        let mod_user_quota_bytes = positive("MOD_USER_QUOTA_BYTES", DEFAULT_MOD_USER_QUOTA_BYTES)?;
        let mod_uploads_per_day = u32::try_from(positive(
            "MOD_UPLOADS_PER_DAY",
            u64::from(DEFAULT_MOD_UPLOADS_PER_DAY),
        )?)
        .map_err(|_| ConfigError::Invalid("MOD_UPLOADS_PER_DAY"))?;

        Ok(Self {
            mod_storage_dir,
            mod_max_upload_bytes,
            mod_user_quota_bytes,
            mod_uploads_per_day,
            discord_client_id,
            discord_client_secret,
            public_base_url,
            database_url,
            admin_discord_ids,
            bind_addr,
            trust_proxy_headers,
            discord_api_base,
        })
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("discord_client_id", &self.discord_client_id)
            .field("discord_client_secret", &"<redacted>")
            .field("public_base_url", &self.public_base_url)
            .field("database_url", &self.database_url)
            .field("admin_discord_ids", &self.admin_discord_ids)
            .field("bind_addr", &self.bind_addr)
            .field("trust_proxy_headers", &self.trust_proxy_headers)
            .field("discord_api_base", &self.discord_api_base)
            .field("mod_storage_dir", &self.mod_storage_dir)
            .field("mod_max_upload_bytes", &self.mod_max_upload_bytes)
            .field("mod_user_quota_bytes", &self.mod_user_quota_bytes)
            .field("mod_uploads_per_day", &self.mod_uploads_per_day)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("DISCORD_CLIENT_ID", "cid"),
            ("DISCORD_CLIENT_SECRET", "s3cret-value"),
            ("PUBLIC_BASE_URL", "https://community.example.com/"),
            ("DATABASE_URL", "sqlite::memory:"),
            ("ADMIN_DISCORD_IDS", "111, 222"),
            ("MOD_STORAGE_DIR", "/var/lib/motogp-community/mods"),
        ])
    }

    fn load(vars: &HashMap<&'static str, &'static str>) -> Result<Config, ConfigError> {
        Config::from_lookup(|name| vars.get(name).map(|v| (*v).to_owned()))
    }

    #[test]
    fn applies_defaults_and_parses_admins() {
        let config = load(&base()).unwrap();
        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:8787");
        assert!(!config.trust_proxy_headers);
        assert_eq!(config.discord_api_base, "https://discord.com/api/v10");
        assert_eq!(config.public_base_url, "https://community.example.com");
        let ids: Vec<_> = config
            .admin_discord_ids
            .iter()
            .map(|i| i.as_str())
            .collect();
        assert_eq!(ids, ["111", "222"]);
    }

    #[test]
    fn mod_hosting_defaults_and_overrides() {
        let config = load(&base()).unwrap();
        assert_eq!(config.mod_max_upload_bytes, 2_147_483_648);
        assert_eq!(config.mod_user_quota_bytes, 10_737_418_240);
        assert_eq!(config.mod_uploads_per_day, 10);
        assert_eq!(
            config.mod_storage_dir,
            std::path::PathBuf::from("/var/lib/motogp-community/mods")
        );
        let mut vars = base();
        vars.insert("MOD_MAX_UPLOAD_BYTES", "1000");
        vars.insert("MOD_USER_QUOTA_BYTES", "5000");
        vars.insert("MOD_UPLOADS_PER_DAY", "3");
        let config = load(&vars).unwrap();
        assert_eq!(
            (
                config.mod_max_upload_bytes,
                config.mod_user_quota_bytes,
                config.mod_uploads_per_day
            ),
            (1000, 5000, 3)
        );
    }

    #[test]
    fn optional_values_override_defaults() {
        let mut vars = base();
        vars.insert("BIND_ADDR", "0.0.0.0:9000");
        vars.insert("TRUST_PROXY_HEADERS", "true");
        vars.insert("DISCORD_API_BASE", "http://127.0.0.1:1234/");
        let config = load(&vars).unwrap();
        assert_eq!(config.bind_addr.port(), 9000);
        assert!(config.trust_proxy_headers);
        assert_eq!(config.discord_api_base, "http://127.0.0.1:1234");
    }

    #[test]
    fn missing_variable_is_named_without_values() {
        for name in [
            "DISCORD_CLIENT_ID",
            "DISCORD_CLIENT_SECRET",
            "PUBLIC_BASE_URL",
            "DATABASE_URL",
            "ADMIN_DISCORD_IDS",
            "MOD_STORAGE_DIR",
        ] {
            let mut vars = base();
            vars.remove(name);
            let error = load(&vars).unwrap_err();
            assert_eq!(error, ConfigError::Missing(name));
            assert!(error.to_string().contains(name));
            assert!(!error.to_string().contains("s3cret-value"));
        }
    }

    #[test]
    fn invalid_values_are_rejected() {
        for (name, value) in [
            ("ADMIN_DISCORD_IDS", "abc"),
            ("BIND_ADDR", "nope"),
            ("TRUST_PROXY_HEADERS", "yes"),
            ("MOD_MAX_UPLOAD_BYTES", "0"),
            ("MOD_USER_QUOTA_BYTES", "ten"),
            ("MOD_UPLOADS_PER_DAY", "-1"),
            ("MOD_UPLOADS_PER_DAY", "99999999999"),
        ] {
            let mut vars = base();
            vars.insert(name, value);
            assert_eq!(load(&vars).unwrap_err(), ConfigError::Invalid(name));
        }
    }

    #[test]
    fn debug_redacts_the_secret() {
        let config = load(&base()).unwrap();
        assert!(!format!("{config:?}").contains("s3cret-value"));
    }
}
