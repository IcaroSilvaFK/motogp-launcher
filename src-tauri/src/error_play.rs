//! Tauri-boundary mapping for the Play, import and removal use-case errors.
//! Only presentation: stable codes plus the application's own messages.
use crate::CommandError;
use motogp_application::{DeployError, ImportBatchError, PlayError, RemoveModError, SessionError};

fn deploy_code(error: &DeployError) -> &'static str {
    match error {
        DeployError::Storage(_) => "deploy_storage_failed",
        DeployError::Io { .. } => "deploy_io_failed",
        DeployError::AlreadyExists { .. } => "deploy_already_exists",
        DeployError::InvalidPath { .. } => "deploy_invalid_path",
        DeployError::UnauthorizedDirectory { .. } => "deploy_unauthorized_directory",
        DeployError::CleanIncomplete { .. } => "deploy_cleanup_incomplete",
    }
}

impl From<&DeployError> for CommandError {
    fn from(error: &DeployError) -> Self {
        CommandError {
            code: deploy_code(error).to_string(),
            message: error.to_string(),
        }
    }
}

impl From<&PlayError> for CommandError {
    fn from(error: &PlayError) -> Self {
        let code = match error {
            PlayError::AlreadyRunning => "already_running",
            PlayError::GameNotConfigured => "game_not_configured",
            PlayError::Launch(_) => "missing_app_id",
            PlayError::Session(SessionError::Launch(_)) => "missing_app_id",
            PlayError::Session(SessionError::Opener(_)) => "open_game_failed",
            PlayError::RecoveryPending { .. } => "recovery_pending",
            PlayError::GameStillRunning => "game_still_running",
            PlayError::ProcessInspection { .. } => "process_inspection_failed",
            PlayError::SessionThread { .. } => "session_thread_failed",
            PlayError::Game(inner) => return CommandError::from(inner),
            PlayError::Profile(inner) => return CommandError::from(inner),
            PlayError::Library(inner) => return CommandError::from(inner),
            PlayError::Deploy(inner) => return CommandError::from(inner),
        };
        CommandError {
            code: code.to_string(),
            message: error.to_string(),
        }
    }
}

impl From<PlayError> for CommandError {
    fn from(error: PlayError) -> Self {
        CommandError::from(&error)
    }
}

impl From<&ImportBatchError> for CommandError {
    fn from(error: &ImportBatchError) -> Self {
        let inner = CommandError::from(&error.error);
        CommandError {
            message: format!(
                "{} ({} import(s) completed before the failure)",
                inner.message,
                error.completed.len()
            ),
            code: inner.code,
        }
    }
}

impl From<ImportBatchError> for CommandError {
    fn from(error: ImportBatchError) -> Self {
        CommandError::from(&error)
    }
}

impl From<&RemoveModError> for CommandError {
    fn from(error: &RemoveModError) -> Self {
        match error {
            RemoveModError::Library(inner) => CommandError::from(inner),
            RemoveModError::Profiles(inner) => CommandError::from(inner),
        }
    }
}

impl From<RemoveModError> for CommandError {
    fn from(error: RemoveModError) -> Self {
        CommandError::from(&error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motogp_application::{AppError, LibraryError};
    use motogp_domain::LaunchError;
    use std::path::PathBuf;

    fn code(error: PlayError) -> String {
        CommandError::from(error).code
    }

    #[test]
    fn play_errors_have_stable_codes() {
        assert_eq!(code(PlayError::AlreadyRunning), "already_running");
        assert_eq!(code(PlayError::GameNotConfigured), "game_not_configured");
        assert_eq!(
            code(PlayError::Launch(LaunchError::MissingAppId)),
            "missing_app_id"
        );
        assert_eq!(
            code(PlayError::Session(SessionError::Launch(
                LaunchError::MissingAppId
            ))),
            "missing_app_id"
        );
        assert_eq!(
            code(PlayError::Session(SessionError::Opener(AppError::Read {
                location: "steam://x".into(),
                detail: "d".into()
            }))),
            "open_game_failed"
        );
        assert_eq!(
            code(PlayError::RecoveryPending { detail: "x".into() }),
            "recovery_pending"
        );
    }

    #[test]
    fn wrapped_errors_reuse_their_own_mapping() {
        let not_unreal = motogp_application::GameError::NotUnrealGame {
            root: PathBuf::from("x"),
        };
        assert_eq!(code(PlayError::Game(not_unreal)), "not_unreal_game");
        let missing = LibraryError::NotFound { id: "a".into() };
        assert_eq!(code(PlayError::Library(missing)), "mod_not_found");
        let profile =
            motogp_application::ProfileServiceError::Profile(motogp_domain::ProfileError::NotFound);
        assert_eq!(code(PlayError::Profile(profile)), "profile_not_found");
    }

    #[test]
    fn deploy_errors_map_every_variant() {
        let cases = [
            (
                DeployError::Storage(AppError::Read {
                    location: "l".into(),
                    detail: "d".into(),
                }),
                "deploy_storage_failed",
            ),
            (
                DeployError::Io {
                    location: "l".into(),
                    detail: "d".into(),
                },
                "deploy_io_failed",
            ),
            (
                DeployError::AlreadyExists { name: "n".into() },
                "deploy_already_exists",
            ),
            (
                DeployError::InvalidPath { path: "p".into() },
                "deploy_invalid_path",
            ),
            (
                DeployError::UnauthorizedDirectory { path: "p".into() },
                "deploy_unauthorized_directory",
            ),
            (
                DeployError::CleanIncomplete {
                    removed: 0,
                    remaining: 1,
                    detail: "d".into(),
                },
                "deploy_cleanup_incomplete",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(code(PlayError::Deploy(error)), expected);
        }
    }

    #[test]
    fn batch_and_removal_errors_keep_the_inner_code() {
        let batch = ImportBatchError {
            completed: vec![],
            failed: PathBuf::from("n.txt"),
            error: LibraryError::NoPakFound,
        };
        let mapped = CommandError::from(batch);
        assert_eq!(mapped.code, "no_pak_found");
        assert!(mapped.message.contains("0 import(s) completed"));

        let library = RemoveModError::Library(LibraryError::NotFound { id: "a".into() });
        assert_eq!(CommandError::from(library).code, "mod_not_found");
        let profiles = RemoveModError::Profiles(motogp_application::ProfileServiceError::Profile(
            motogp_domain::ProfileError::NotFound,
        ));
        assert_eq!(CommandError::from(profiles).code, "profile_not_found");
    }
}
