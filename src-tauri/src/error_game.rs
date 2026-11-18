//! Error mapping at the Tauri boundary for game detection errors.
use crate::CommandError;
use motogp_application::GameError;

impl From<&GameError> for CommandError {
    fn from(error: &GameError) -> Self {
        todo!("{error:?}")
    }
}

impl From<GameError> for CommandError {
    fn from(error: GameError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::AppError;
    use std::path::PathBuf;

    #[test]
    fn not_unreal_game_maps_to_a_stable_code_and_message() {
        let mapped = CommandError::from(GameError::NotUnrealGame {
            root: PathBuf::from("D:/Games/Foo"),
        });
        assert_eq!(mapped.code, "not_unreal_game");
        assert!(mapped
            .message
            .contains("not an Unreal game: Content/Paks not found"));
        assert!(mapped.message.contains("Foo"));
    }

    #[test]
    fn storage_errors_reuse_the_app_error_mapping() {
        let app = AppError::Read {
            location: "lib/steamapps".into(),
            detail: "denied".into(),
        };
        let expected = CommandError::from(app.clone());
        assert_eq!(CommandError::from(GameError::Storage(app)), expected);
    }
}
