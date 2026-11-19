use crate::AppError;
use motogp_domain::Config;

/// Port: persistence of the user configuration.
pub trait ConfigRepository {
    /// Missing storage yields `Config::default()`; invalid storage is an error.
    fn load(&self) -> Result<Config, AppError>;
    /// Persists the configuration without destroying the previous version on failure.
    fn save(&self, config: &Config) -> Result<(), AppError>;
}
