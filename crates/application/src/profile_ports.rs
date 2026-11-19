use crate::AppError;
use motogp_domain::Profile;

/// Port: persistence of the ordered profile collection.
pub trait ProfileRepository {
    /// Missing storage yields an empty list; invalid storage is an error.
    fn load(&self) -> Result<Vec<Profile>, AppError>;
    /// Replaces the stored collection without destroying the previous version on failure.
    fn save(&self, profiles: &[Profile]) -> Result<(), AppError>;
}

/// Port: time source used to build new profile IDs (`p<unix_millis>`).
/// Replaceable in tests so the core never reads the real clock.
pub trait Clock {
    fn now_millis(&self) -> u64;
}
