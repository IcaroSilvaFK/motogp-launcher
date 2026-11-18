/// A mod stored in the local library: pure model, no I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry {
    /// 12 lowercase hex characters derived from the SHA-256 of the `.pak`.
    pub id: String,
    /// Display name (the base name without the final `_P`).
    pub name: String,
    /// File names (never paths) of the mod, suffix `_P` preserved.
    pub files: Vec<String>,
    /// Sum of the sizes of `files`.
    pub size_bytes: u64,
}

/// Length of a mod identifier in hex characters.
pub const MOD_ID_LEN: usize = 12;

/// Mod ID: the first 12 characters of the content hash, lowercased.
pub fn mod_id_from_hash(_hash_hex: &str) -> String {
    todo!()
}

/// `true` for exactly 12 lowercase hexadecimal characters.
pub fn is_valid_mod_id(_id: &str) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_id_is_the_first_twelve_lowercase_hex_characters() {
        let hash = "A1B2C3D4E5F60718293A4B5C6D7E8F90A1B2C3D4E5F60718293A4B5C6D7E8F90";
        assert_eq!(mod_id_from_hash(hash), "a1b2c3d4e5f6");
    }

    #[test]
    fn valid_ids_are_twelve_lowercase_hex_characters() {
        assert!(is_valid_mod_id("a1b2c3d4e5f6"));
        assert!(is_valid_mod_id("000000000000"));
    }

    #[test]
    fn invalid_ids_are_rejected() {
        for id in [
            "",
            "a1b2c3d4e5f",
            "a1b2c3d4e5f67",
            "A1B2C3D4E5F6",
            "a1b2c3d4e5fg",
            "..",
            "../a1b2c3d4",
            "a1b2c3d4e5f\\",
        ] {
            assert!(!is_valid_mod_id(id), "{id:?} should be invalid");
        }
    }

    #[test]
    fn entry_keeps_its_values() {
        let entry = ModEntry {
            id: "a1b2c3d4e5f6".into(),
            name: "A".into(),
            files: vec!["A_P.pak".into()],
            size_bytes: 10,
        };
        assert_eq!(entry.clone(), entry);
    }
}
