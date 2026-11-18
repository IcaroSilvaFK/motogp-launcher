use std::path::PathBuf;

/// User configuration: pure model, no I/O and no serialization concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Game installation directory, when already chosen.
    pub game_dir: Option<PathBuf>,
    /// Identifier of the active profile, when one is selected.
    pub active_profile: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        todo!("red: default config")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_no_game_dir_and_no_active_profile() {
        let config = Config::default();
        assert_eq!(config.game_dir, None);
        assert_eq!(config.active_profile, None);
    }

    #[test]
    fn config_keeps_values_with_spaces_and_accents() {
        let config = Config {
            game_dir: Some(PathBuf::from("Usuário Padrão/Program Files (x86)")),
            active_profile: Some("p1".to_string()),
        };
        assert_eq!(config.clone(), config);
        assert_ne!(config, Config::default());
    }
}
