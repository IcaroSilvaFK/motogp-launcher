use crate::{AppError, Clock, ConfigRepository, ProfileRepository};
use motogp_domain::{normalize_name, Config, Profile, ProfileError, VANILLA_ID};
use std::fmt;

/// Failure of a profile use case: a business rule or the storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileServiceError {
    Profile(ProfileError),
    Storage(AppError),
}

impl From<ProfileError> for ProfileServiceError {
    fn from(error: ProfileError) -> Self {
        ProfileServiceError::Profile(error)
    }
}

impl From<AppError> for ProfileServiceError {
    fn from(error: AppError) -> Self {
        ProfileServiceError::Storage(error)
    }
}

impl fmt::Display for ProfileServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileServiceError::Profile(error) => error.fmt(f),
            ProfileServiceError::Storage(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ProfileServiceError {}

/// Use cases for ordered profiles, built on `ProfileRepository` and `Clock`.
pub struct ProfileService<R: ProfileRepository, C: Clock> {
    repository: R,
    clock: C,
}

impl<R: ProfileRepository, C: Clock> ProfileService<R, C> {
    pub fn new(repository: R, clock: C) -> Self {
        Self { repository, clock }
    }

    /// Loads the collection; an empty one is seeded with Vanilla and persisted.
    /// Invalid storage is an error and is never reseeded.
    fn load_seeded(&self) -> Result<Vec<Profile>, ProfileServiceError> {
        let profiles = self.repository.load()?;
        if profiles.is_empty() {
            let seeded = vec![Profile::vanilla()];
            self.repository.save(&seeded)?;
            return Ok(seeded);
        }
        Ok(profiles)
    }

    /// Lists the profiles; an empty store is seeded with Vanilla and persisted.
    pub fn list(&self) -> Result<Vec<Profile>, ProfileServiceError> {
        self.load_seeded()
    }

    pub fn get(&self, id: &str) -> Result<Profile, ProfileServiceError> {
        self.load_seeded()?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| ProfileError::NotFound.into())
    }

    /// Creates a profile with a fresh `p<unix_millis>` ID and no mods.
    pub fn create(&self, name: &str) -> Result<Profile, ProfileServiceError> {
        let name = normalize_name(name)?;
        let mut profiles = self.load_seeded()?;
        let profile = Profile::new(self.next_id(&profiles), &name)?;
        profiles.push(profile.clone());
        self.repository.save(&profiles)?;
        Ok(profile)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<Profile, ProfileServiceError> {
        let name = normalize_name(name)?;
        self.update(id, |profile| profile.rename(&name))
    }

    /// Deletes a profile; Vanilla is always refused. When the deleted profile is
    /// the active one, the config falls back to Vanilla (persisted before the
    /// profile is removed, so a failure never leaves a dangling active ID).
    pub fn delete(
        &self,
        id: &str,
        config: &impl ConfigRepository,
    ) -> Result<(), ProfileServiceError> {
        if id == VANILLA_ID {
            return Err(ProfileError::VanillaProtected.into());
        }
        let mut profiles = self.load_seeded()?;
        let before = profiles.len();
        profiles.retain(|profile| profile.id != id);
        if profiles.len() == before {
            return Err(ProfileError::NotFound.into());
        }
        let current = config.load()?;
        let was_active = current.active_profile.as_deref() == Some(id);
        if was_active {
            let fallback = Config {
                active_profile: Some(VANILLA_ID.to_string()),
                ..current.clone()
            };
            config.save(&fallback)?;
        }
        if let Err(error) = self.repository.save(&profiles) {
            if was_active {
                // Best effort: restore the previous active profile.
                let _ = config.save(&current);
            }
            return Err(error.into());
        }
        Ok(())
    }

    /// Selects the active profile after checking that it exists.
    pub fn set_active(
        &self,
        id: &str,
        config: &impl ConfigRepository,
    ) -> Result<(), ProfileServiceError> {
        self.get(id)?;
        let current = config.load()?;
        config.save(&Config {
            active_profile: Some(id.to_string()),
            ..current
        })?;
        Ok(())
    }

    /// Replaces the mods of a profile, preserving the given order.
    pub fn set_mods(&self, id: &str, mod_ids: Vec<String>) -> Result<Profile, ProfileServiceError> {
        self.update(id, |profile| {
            profile.mod_ids = mod_ids;
            Ok(())
        })
    }

    /// Removes `mod_id` from every profile (used after a mod leaves the library).
    pub fn remove_mod_everywhere(&self, mod_id: &str) -> Result<(), ProfileServiceError> {
        let mut profiles = self.load_seeded()?;
        let changed = profiles
            .iter_mut()
            .fold(false, |acc, profile| profile.remove_mod(mod_id) | acc);
        if changed {
            self.repository.save(&profiles)?;
        }
        Ok(())
    }

    /// Applies `change` to one profile and persists the collection.
    fn update(
        &self,
        id: &str,
        change: impl FnOnce(&mut Profile) -> Result<(), ProfileError>,
    ) -> Result<Profile, ProfileServiceError> {
        let mut profiles = self.load_seeded()?;
        let profile = profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .ok_or(ProfileError::NotFound)?;
        change(profile)?;
        let updated = profile.clone();
        self.repository.save(&profiles)?;
        Ok(updated)
    }

    /// `p<unix_millis>`, bumped until it is free so rapid creations never collide.
    fn next_id(&self, profiles: &[Profile]) -> String {
        let mut millis = self.clock.now_millis();
        loop {
            let id = format!("p{millis}");
            if profiles.iter().all(|profile| profile.id != id) {
                return id;
            }
            millis += 1;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use motogp_domain::Config;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct FakeRepository {
        stored: RefCell<Vec<Profile>>,
        saves: Cell<usize>,
        load_error: RefCell<Option<AppError>>,
        save_error: RefCell<Option<AppError>>,
    }

    #[derive(Default)]
    struct FakeConfigRepository {
        stored: RefCell<Config>,
        saves: Cell<usize>,
        save_error: RefCell<Option<AppError>>,
    }

    impl ConfigRepository for FakeConfigRepository {
        fn load(&self) -> Result<Config, AppError> {
            Ok(self.stored.borrow().clone())
        }

        fn save(&self, config: &Config) -> Result<(), AppError> {
            if let Some(error) = self.save_error.borrow().clone() {
                return Err(error);
            }
            self.saves.set(self.saves.get() + 1);
            *self.stored.borrow_mut() = config.clone();
            Ok(())
        }
    }

    fn config_with_active(active: Option<&str>) -> FakeConfigRepository {
        let config = FakeConfigRepository::default();
        *config.stored.borrow_mut() = Config {
            game_dir: Some("C:/Game".into()),
            active_profile: active.map(str::to_string),
        };
        config
    }

    fn write_failure() -> AppError {
        AppError::Write {
            location: "x.json".into(),
            detail: "denied".into(),
        }
    }

    impl ProfileRepository for FakeRepository {
        fn load(&self) -> Result<Vec<Profile>, AppError> {
            match self.load_error.borrow().clone() {
                Some(error) => Err(error),
                None => Ok(self.stored.borrow().clone()),
            }
        }

        fn save(&self, profiles: &[Profile]) -> Result<(), AppError> {
            if let Some(error) = self.save_error.borrow().clone() {
                return Err(error);
            }
            self.saves.set(self.saves.get() + 1);
            *self.stored.borrow_mut() = profiles.to_vec();
            Ok(())
        }
    }

    struct FixedClock(u64);

    impl Clock for FixedClock {
        fn now_millis(&self) -> u64 {
            self.0
        }
    }

    fn service() -> ProfileService<FakeRepository, FixedClock> {
        ProfileService::new(FakeRepository::default(), FixedClock(1_700_000_000_000))
    }

    fn stored(service: &ProfileService<FakeRepository, FixedClock>) -> Vec<Profile> {
        service.repository.stored.borrow().clone()
    }

    fn mods(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_store_seeds_vanilla() {
        let service = service();
        let listed = service.list().unwrap();
        assert_eq!(listed, vec![Profile::vanilla()]);
        assert_eq!(stored(&service), vec![Profile::vanilla()]);
        // A second listing does not save again.
        service.list().unwrap();
        assert_eq!(service.repository.saves.get(), 1);
    }

    #[test]
    fn create_then_list() {
        let service = service();
        let created = service.create("  Liveries 2025 ").unwrap();
        assert_eq!(created.name, "Liveries 2025");
        assert_eq!(created.id, "p1700000000000");
        assert!(created.mod_ids.is_empty());
        let listed = service.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed[0].is_vanilla(), "Vanilla is not lost");
        assert_eq!(listed[1], created);
        assert_eq!(stored(&service), listed);
    }

    #[test]
    fn set_mods_preserves_order() {
        let service = service();
        let created = service.create("A").unwrap();
        let updated = service
            .set_mods(&created.id, mods(&["b", "a", "c"]))
            .unwrap();
        assert_eq!(updated.mod_ids, ["b", "a", "c"]);
        assert_eq!(service.get(&created.id).unwrap().mod_ids, ["b", "a", "c"]);
        assert_eq!(stored(&service)[1].mod_ids, ["b", "a", "c"]);
    }

    #[test]
    fn delete_removes_profile() {
        let service = service();
        let created = service.create("A").unwrap();
        service
            .delete(&created.id, &FakeConfigRepository::default())
            .unwrap();
        assert_eq!(stored(&service), vec![Profile::vanilla()]);
        assert_eq!(
            service.get(&created.id),
            Err(ProfileServiceError::Profile(ProfileError::NotFound))
        );
    }

    #[test]
    fn remove_mod_everywhere_strips_from_all_profiles() {
        let service = service();
        let one = service.create("One").unwrap();
        service.set_mods(&one.id, mods(&["c", "a", "b"])).unwrap();
        // Distinct ID source for the second profile.
        let two = service.create("Two").unwrap();
        service.set_mods(&two.id, mods(&["a", "d"])).unwrap();
        service.remove_mod_everywhere("a").unwrap();
        let profiles = stored(&service);
        assert!(profiles.iter().all(|p| !p.mod_ids.contains(&"a".into())));
        assert_eq!(profiles[1].mod_ids, ["c", "b"]);
        assert_eq!(profiles[2].mod_ids, ["d"]);
    }

    #[test]
    fn blank_name_is_rejected() {
        let service = service();
        let created = service.create("Keep").unwrap();
        let saves = service.repository.saves.get();
        let before = stored(&service);
        let empty = Err(ProfileServiceError::Profile(ProfileError::EmptyName));
        assert_eq!(service.create("   "), empty);
        assert_eq!(service.rename(&created.id, "\t "), empty);
        assert_eq!(stored(&service), before);
        assert_eq!(service.repository.saves.get(), saves);
    }

    #[test]
    fn vanilla_cannot_be_deleted() {
        let service = service();
        service.list().unwrap();
        assert_eq!(
            service.delete("vanilla", &FakeConfigRepository::default()),
            Err(ProfileServiceError::Profile(ProfileError::VanillaProtected))
        );
        assert_eq!(stored(&service), vec![Profile::vanilla()]);
    }

    #[test]
    fn rename_persists_new_name() {
        let service = service();
        let created = service.create("Old").unwrap();
        let renamed = service.rename(&created.id, " Race ").unwrap();
        assert_eq!(renamed.name, "Race");
        assert_eq!(stored(&service)[1].name, "Race");
    }

    #[test]
    fn unknown_profile_returns_error() {
        let service = service();
        let not_found = ProfileServiceError::Profile(ProfileError::NotFound);
        assert_eq!(service.get("nope"), Err(not_found.clone()));
        assert_eq!(service.rename("nope", "X"), Err(not_found.clone()));
        let config = FakeConfigRepository::default();
        assert_eq!(service.delete("nope", &config), Err(not_found.clone()));
        assert_eq!(service.set_mods("nope", mods(&["a"])), Err(not_found));
    }

    #[test]
    fn profile_ids_do_not_collide() {
        // The clock never advances: three rapid creations must still differ.
        let service = service();
        let ids: Vec<String> = (0..3)
            .map(|i| service.create(&format!("P{i}")).unwrap().id)
            .collect();
        assert_eq!(ids[0], "p1700000000000");
        assert_eq!(ids[1], "p1700000000001");
        assert_eq!(ids[2], "p1700000000002");
        assert_eq!(service.list().unwrap().len(), 4);
    }

    #[test]
    fn invalid_storage_is_propagated_and_not_reseeded() {
        let service = service();
        let corrupted = AppError::Corrupted {
            location: "profiles.json".into(),
            detail: "expected value".into(),
        };
        *service.repository.load_error.borrow_mut() = Some(corrupted.clone());
        assert_eq!(service.list(), Err(ProfileServiceError::Storage(corrupted)));
        assert_eq!(service.repository.saves.get(), 0);
    }

    #[test]
    fn deleting_the_active_profile_falls_back_to_vanilla() {
        let service = service();
        let created = service.create("A").unwrap();
        let config = config_with_active(Some(&created.id));
        service.delete(&created.id, &config).unwrap();
        let saved = config.stored.borrow().clone();
        assert_eq!(saved.active_profile.as_deref(), Some("vanilla"));
        assert_eq!(
            saved.game_dir,
            Some("C:/Game".into()),
            "other settings kept"
        );
        assert_eq!(stored(&service), vec![Profile::vanilla()]);
    }

    #[test]
    fn deleting_an_inactive_profile_leaves_the_config_untouched() {
        let service = service();
        let created = service.create("A").unwrap();
        let config = config_with_active(Some("vanilla"));
        service.delete(&created.id, &config).unwrap();
        assert_eq!(config.saves.get(), 0);
        let active = config.stored.borrow().active_profile.clone();
        assert_eq!(active.as_deref(), Some("vanilla"));
    }

    #[test]
    fn delete_propagates_profile_save_failure_without_touching_the_config() {
        let service = service();
        let created = service.create("A").unwrap();
        let config = config_with_active(Some(&created.id));
        *service.repository.save_error.borrow_mut() = Some(write_failure());
        assert_eq!(
            service.delete(&created.id, &config),
            Err(ProfileServiceError::Storage(write_failure()))
        );
        let active = config.stored.borrow().active_profile.clone();
        assert_eq!(active.as_deref(), Some(created.id.as_str()));
    }

    #[test]
    fn delete_propagates_config_save_failure_and_keeps_the_profile() {
        let service = service();
        let created = service.create("A").unwrap();
        let config = config_with_active(Some(&created.id));
        *config.save_error.borrow_mut() = Some(write_failure());
        assert_eq!(
            service.delete(&created.id, &config),
            Err(ProfileServiceError::Storage(write_failure()))
        );
        assert_eq!(stored(&service).len(), 2, "profile kept when config fails");
    }

    #[test]
    fn error_display_delegates_to_the_cause() {
        let rule = ProfileServiceError::from(ProfileError::EmptyName);
        assert_eq!(rule.to_string(), "empty profile name");
        let storage = ProfileServiceError::from(AppError::Read {
            location: "profiles.json".into(),
            detail: "denied".into(),
        });
        assert!(storage.to_string().contains("profiles.json"));
    }
}
