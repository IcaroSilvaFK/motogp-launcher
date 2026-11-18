use std::fmt;

/// Fixed ID of the protected "no mods" profile.
pub const VANILLA_ID: &str = "vanilla";

/// Pure domain errors for profiles (no I/O, no serde).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    /// The profile name is empty after trimming.
    EmptyName,
    /// No profile has the requested ID.
    NotFound,
    /// The Vanilla profile cannot be deleted.
    VanillaProtected,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ProfileError::EmptyName => "empty profile name",
            ProfileError::NotFound => "profile not found",
            ProfileError::VanillaProtected => "the Vanilla profile cannot be deleted",
        };
        f.write_str(text)
    }
}

impl std::error::Error for ProfileError {}

/// Ordered mod selection. `mod_ids` order is the load order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub mod_ids: Vec<String>,
}

/// Trims `name`; a blank result is `ProfileError::EmptyName`.
pub fn normalize_name(name: &str) -> Result<String, ProfileError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Err(ProfileError::EmptyName)
    } else {
        Ok(trimmed.to_string())
    }
}

impl Profile {
    /// New profile without mods; the name is trimmed and must not be blank.
    pub fn new(id: impl Into<String>, name: &str) -> Result<Profile, ProfileError> {
        Ok(Profile {
            id: id.into(),
            name: normalize_name(name)?,
            mod_ids: Vec::new(),
        })
    }

    /// The built-in Vanilla profile: ID `vanilla`, name `Vanilla`, no mods.
    pub fn vanilla() -> Profile {
        Profile {
            id: VANILLA_ID.to_string(),
            name: "Vanilla".to_string(),
            mod_ids: Vec::new(),
        }
    }

    pub fn is_vanilla(&self) -> bool {
        self.id == VANILLA_ID
    }

    /// Renames in place; a blank name leaves the profile unchanged.
    pub fn rename(&mut self, name: &str) -> Result<(), ProfileError> {
        self.name = normalize_name(name)?;
        Ok(())
    }

    /// Removes every occurrence of `mod_id`, keeping the order of the others.
    /// Returns whether anything was removed.
    pub fn remove_mod(&mut self, mod_id: &str) -> bool {
        let before = self.mod_ids.len();
        self.mod_ids.retain(|id| id != mod_id);
        self.mod_ids.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_name_trims_and_rejects_blank() {
        assert_eq!(normalize_name("  Corrida  "), Ok("Corrida".to_string()));
        assert_eq!(normalize_name(""), Err(ProfileError::EmptyName));
        assert_eq!(normalize_name(" \t\n "), Err(ProfileError::EmptyName));
    }

    #[test]
    fn new_profile_trims_name_and_starts_without_mods() {
        let profile = Profile::new("p1", "  Pinturas 2025 ").unwrap();
        assert_eq!(profile.id, "p1");
        assert_eq!(profile.name, "Pinturas 2025");
        assert!(profile.mod_ids.is_empty());
    }

    #[test]
    fn new_profile_rejects_blank_name() {
        assert_eq!(Profile::new("p1", "   "), Err(ProfileError::EmptyName));
    }

    #[test]
    fn vanilla_has_fixed_id_and_no_mods() {
        let vanilla = Profile::vanilla();
        assert_eq!(vanilla.id, VANILLA_ID);
        assert_eq!(vanilla.id, "vanilla");
        assert_eq!(vanilla.name, "Vanilla");
        assert!(vanilla.mod_ids.is_empty());
        assert!(vanilla.is_vanilla());
        assert!(!Profile::new("p1", "x").unwrap().is_vanilla());
    }

    #[test]
    fn rename_updates_name_and_blank_keeps_it() {
        let mut profile = Profile::new("p1", "Old").unwrap();
        assert_eq!(profile.rename(" New "), Ok(()));
        assert_eq!(profile.name, "New");
        assert_eq!(profile.rename("  "), Err(ProfileError::EmptyName));
        assert_eq!(profile.name, "New");
    }

    #[test]
    fn remove_mod_strips_every_occurrence_and_keeps_order() {
        let mut profile = Profile::new("p1", "x").unwrap();
        profile.mod_ids = ["c", "a", "b", "a"].map(String::from).to_vec();
        assert!(profile.remove_mod("a"));
        assert_eq!(profile.mod_ids, ["c", "b"]);
        assert!(!profile.remove_mod("zzz"));
        assert_eq!(profile.mod_ids, ["c", "b"]);
    }

    #[test]
    fn error_messages_are_user_readable() {
        assert_eq!(ProfileError::EmptyName.to_string(), "empty profile name");
        assert_eq!(ProfileError::NotFound.to_string(), "profile not found");
        assert!(ProfileError::VanillaProtected
            .to_string()
            .contains("Vanilla"));
    }
}
