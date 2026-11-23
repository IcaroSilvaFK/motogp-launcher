//! `ProfileRepository` backed by `profiles.json` and the real `Clock`.
use crate::store::{load_json, save_json};
use motogp_application::{AppError, Clock, ProfileRepository};
use motogp_domain::{normalize_load_order, normalize_name, Profile, ProfileError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Serialization shape of one entry of `profiles.json` (kept out of the domain).
#[derive(Debug, Serialize, Deserialize)]
struct ProfileDto {
    id: String,
    name: String,
    #[serde(default)]
    mod_ids: Vec<String>,
}

impl TryFrom<ProfileDto> for Profile {
    type Error = ProfileError;

    fn try_from(dto: ProfileDto) -> Result<Self, Self::Error> {
        Ok(Profile {
            id: dto.id,
            name: normalize_name(&dto.name)?,
            // Migration: legacy files keep their order; only duplicates collapse.
            mod_ids: normalize_load_order(dto.mod_ids),
        })
    }
}

impl From<&Profile> for ProfileDto {
    fn from(profile: &Profile) -> Self {
        ProfileDto {
            id: profile.id.clone(),
            name: profile.name.clone(),
            mod_ids: profile.mod_ids.clone(),
        }
    }
}

/// `ProfileRepository` backed by a JSON array file.
#[derive(Debug, Clone)]
pub struct JsonProfileStore {
    path: PathBuf,
}

impl JsonProfileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl ProfileRepository for JsonProfileStore {
    fn load(&self) -> Result<Vec<Profile>, AppError> {
        let dtos: Vec<ProfileDto> = load_json(&self.path)?;
        let corrupted = |detail: String| AppError::Corrupted {
            location: self.path.display().to_string(),
            detail,
        };
        let mut profiles: Vec<Profile> = Vec::with_capacity(dtos.len());
        for dto in dtos {
            let profile = Profile::try_from(dto).map_err(|e| corrupted(e.to_string()))?;
            if profiles.iter().any(|other| other.id == profile.id) {
                return Err(corrupted(format!(
                    "duplicate profile id \"{}\"",
                    profile.id
                )));
            }
            profiles.push(profile);
        }
        Ok(profiles)
    }

    fn save(&self, profiles: &[Profile]) -> Result<(), AppError> {
        let dtos: Vec<ProfileDto> = profiles.iter().map(ProfileDto::from).collect();
        save_json(&self.path, &dtos)
    }
}

/// Real time source (unix epoch milliseconds).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::ProfileService;
    use std::fs;
    use tempfile::tempdir;

    fn profile(id: &str, name: &str, mods: &[&str]) -> Profile {
        Profile {
            id: id.into(),
            name: name.into(),
            mod_ids: mods.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn profiles_roundtrip_preserves_order() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data").join("profiles.json");
        let store = JsonProfileStore::new(&path);
        let profiles = vec![
            Profile::vanilla(),
            profile("p2", "Liveries 2025", &["b", "a", "c"]),
            profile("p1", "Race", &["z", "y"]),
        ];
        store.save(&profiles).unwrap();
        assert_eq!(store.load(), Ok(profiles));
    }

    #[test]
    fn missing_file_loads_as_empty() {
        let dir = tempdir().unwrap();
        let store = JsonProfileStore::new(dir.path().join("profiles.json"));
        assert_eq!(store.load(), Ok(vec![]));
    }

    #[test]
    fn missing_mod_ids_field_defaults_to_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        fs::write(&path, br#"[{"id":"p1","name":"Old"}]"#).unwrap();
        let store = JsonProfileStore::new(&path);
        assert_eq!(store.load(), Ok(vec![profile("p1", "Old", &[])]));
    }

    #[test]
    fn legacy_file_keeps_its_order_and_unknown_ids() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let content = br#"[{"id":"p1","name":"Old","mod_ids":["z","y","x"]}]"#;
        fs::write(&path, content).unwrap();
        let store = JsonProfileStore::new(&path);
        assert_eq!(
            store.load(),
            Ok(vec![profile("p1", "Old", &["z", "y", "x"])])
        );
        assert_eq!(fs::read(&path).unwrap(), content, "not rewritten on load");
    }

    #[test]
    fn duplicate_mod_ids_collapse_to_the_first_occurrence_on_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let content = br#"[{"id":"p1","name":"Old","mod_ids":["a","b","a"]}]"#;
        fs::write(&path, content).unwrap();
        let store = JsonProfileStore::new(&path);
        assert_eq!(store.load(), Ok(vec![profile("p1", "Old", &["a", "b"])]));
        assert_eq!(fs::read(&path).unwrap(), content, "not rewritten on load");
    }

    #[test]
    fn invalid_profiles_json_is_not_reseeded() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        for content in [&b"{ not json"[..], &b""[..]] {
            fs::write(&path, content).unwrap();
            let service = ProfileService::new(JsonProfileStore::new(&path), SystemClock);
            assert!(service.list().is_err());
            assert_eq!(fs::read(&path).unwrap(), content);
        }
    }

    #[test]
    fn blank_profile_name_is_corrupted_and_left_untouched() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let content = br#"[{"id":"p1","name":"   ","mod_ids":[]}]"#;
        fs::write(&path, content).unwrap();
        let store = JsonProfileStore::new(&path);
        assert!(matches!(store.load(), Err(AppError::Corrupted { .. })));
        let service = ProfileService::new(store, SystemClock);
        assert!(service.list().is_err());
        assert_eq!(fs::read(&path).unwrap(), content, "file never reseeded");
    }

    #[test]
    fn duplicate_profile_ids_are_corrupted_and_left_untouched() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        let content = br#"[{"id":"p1","name":"A"},{"id":"p1","name":"B"}]"#;
        fs::write(&path, content).unwrap();
        let store = JsonProfileStore::new(&path);
        assert!(matches!(store.load(), Err(AppError::Corrupted { .. })));
        let service = ProfileService::new(store, SystemClock);
        assert!(service.list().is_err());
        assert_eq!(fs::read(&path).unwrap(), content, "file never reseeded");
    }

    #[test]
    fn empty_collection_is_seeded_and_persisted() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("profiles.json");
        fs::write(&path, b"[]").unwrap();
        let store = JsonProfileStore::new(&path);
        let service = ProfileService::new(store.clone(), SystemClock);
        assert_eq!(service.list().unwrap(), vec![Profile::vanilla()]);
        assert_eq!(store.load(), Ok(vec![Profile::vanilla()]));
    }

    #[test]
    fn system_clock_returns_current_unix_millis() {
        // 2020-01-01 in millis; the real clock is only exercised in infrastructure.
        assert!(SystemClock.now_millis() > 1_577_836_800_000);
    }

    #[test]
    fn rapid_creations_with_the_real_clock_do_not_collide() {
        let dir = tempdir().unwrap();
        let service = ProfileService::new(
            JsonProfileStore::new(dir.path().join("profiles.json")),
            SystemClock,
        );
        let ids: Vec<String> = (0..5)
            .map(|i| service.create(&format!("P{i}")).unwrap().id)
            .collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 5);
        assert!(ids.iter().all(|id| id.starts_with('p')));
    }
}
