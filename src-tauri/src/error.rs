//! Error mapping at the Tauri boundary: application errors become a
//! serializable `CommandError`. All user-facing text lives in `user_message`
//! so translating it is a one-place change.
use motogp_application::AppError;
use serde::Serialize;

/// Serializable error returned by Tauri commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    /// Stable machine-readable code the frontend can switch on.
    pub code: String,
    /// Human-readable message for the user.
    pub message: String,
}

/// The only place with user-facing text (currently English).
fn user_message(_error: &AppError) -> String {
    todo!("red")
}

impl From<&AppError> for CommandError {
    fn from(_error: &AppError) -> Self {
        todo!("red")
    }
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(kind: &str) -> AppError {
        let (location, detail) = ("Usuário Padrão/config.json".into(), "boom".into());
        match kind {
            "read" => AppError::Read { location, detail },
            "write" => AppError::Write { location, detail },
            _ => AppError::Corrupted { location, detail },
        }
    }

    #[test]
    fn maps_each_variant_to_a_stable_code() {
        assert_eq!(CommandError::from(err("read")).code, "read_failed");
        assert_eq!(CommandError::from(err("write")).code, "write_failed");
        assert_eq!(CommandError::from(err("corrupted")).code, "data_corrupted");
    }

    #[test]
    fn message_mentions_the_location() {
        for kind in ["read", "write", "corrupted"] {
            let mapped = CommandError::from(err(kind));
            assert!(
                mapped.message.contains("Usuário Padrão/config.json"),
                "{kind}: {}",
                mapped.message
            );
        }
    }

    #[test]
    fn corrupted_message_says_the_file_was_not_modified() {
        let mapped = CommandError::from(err("corrupted"));
        assert!(mapped.message.contains("not modified"));
    }

    #[test]
    fn serializes_as_code_and_message() {
        let value = serde_json::to_value(CommandError::from(err("write"))).unwrap();
        assert_eq!(value["code"], "write_failed");
        assert!(value["message"].is_string());
    }
}
