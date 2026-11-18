//! `ProfileRepository` backed by `profiles.json` and the real `Clock`.
use crate::store::{load_json, save_json};
use motogp_application::{AppError, Clock, ProfileRepository};
use motogp_domain::Profile;
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

impl From<ProfileDto> for Profile {
    fn from(dto: ProfileDto) -> Self {
        Profile {
            id: dto.id,
            name: dto.name,
            mod_ids: dto.mod_ids,
        }
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
        Ok(dtos.into_iter().map(Profile::from).collect())
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
        let path = dir.path().join("dados").join("profiles.json");
        let store = JsonProfileStore::new(&path);
        let profiles = vec![
            Profile::vanilla(),
            profile("p2", "Pinturas 2025", &["b", "a", "c"]),
            profile("p1", "Corrida", &["z", "y"]),
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
