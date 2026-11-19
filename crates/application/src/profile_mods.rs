//! Turns a profile into the ordered list of mods to deploy.
use crate::{
    Clock, DeployMod, ModPlanner, ModRepository, PlayError, ProfileRepository, ProfileService,
};
use motogp_domain::{ModEntry, Profile};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Keeps the profile order and skips IDs that are no longer in the library.
/// Each mod's files are read from `library_dir/<id>`.
pub fn resolve_profile_mods(
    profile: &Profile,
    library: &[ModEntry],
    library_dir: &Path,
) -> Vec<DeployMod> {
    profile
        .mod_ids
        .iter()
        .filter_map(|id| library.iter().find(|entry| entry.id == *id))
        .map(|entry| DeployMod {
            id: entry.id.clone(),
            source_dir: library_dir.join(&entry.id),
            files: entry.files.clone(),
        })
        .collect()
}

/// `ModPlanner` backed by the profile service and the library index.
pub struct ProfileModPlanner<R: ProfileRepository, C: Clock, M> {
    profiles: Arc<ProfileService<R, C>>,
    library: M,
    library_dir: PathBuf,
}

impl<R: ProfileRepository, C: Clock, M: ModRepository> ProfileModPlanner<R, C, M> {
    pub fn new(profiles: Arc<ProfileService<R, C>>, library: M, library_dir: PathBuf) -> Self {
        Self {
            profiles,
            library,
            library_dir,
        }
    }
}

impl<R, C, M> ModPlanner for ProfileModPlanner<R, C, M>
where
    R: ProfileRepository + Send + Sync,
    C: Clock + Send + Sync,
    M: ModRepository + Send + Sync,
{
    fn plan(&self, profile_id: &str) -> Result<Vec<DeployMod>, PlayError> {
        let profile = self.profiles.get(profile_id)?;
        let library = self.library.list()?;
        Ok(resolve_profile_mods(&profile, &library, &self.library_dir))
    }
}
