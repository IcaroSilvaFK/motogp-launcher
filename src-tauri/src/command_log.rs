//! Central logging of the errors Tauri commands return (issue #167).
//!
//! Every command goes through `mutate`, `read_blocking` or `logged`, which call
//! `record` once on failure. Only the command name, the stable error code and
//! the redacted message are logged: never the arguments, which may hold session
//! tokens or OAuth codes.
use crate::CommandError;
use motogp_application::redact_secrets;

/// Expected outcomes of user actions or the network; not worth an error line.
const BENIGN_CODES: &[&str] = &[
    "cancelled",
    "download_cancelled",
    "community_sign_in_denied",
    "community_signed_out",
    "not_signed_in",
    "session_expired",
    "offline",
    "community_offline",
];

pub(crate) fn is_benign(code: &str) -> bool {
    BENIGN_CODES.contains(&code)
}

/// The log line for a failed command, or `None` for a benign code.
pub(crate) fn error_line(command: &str, error: &CommandError) -> Option<String> {
    if is_benign(&error.code) {
        return None;
    }
    Some(format!(
        "command {command} failed [{}]: {}",
        error.code,
        redact_secrets(&error.message)
    ))
}

/// Derives the command name from the type name of the job closure a command
/// hands to a helper (`crate::commands::play::{{closure}}` -> `play`).
pub(crate) fn command_name(type_name: &str) -> &str {
    let path = type_name.split("::{{closure}}").next().unwrap_or(type_name);
    let path = path.split('<').next().unwrap_or(path);
    path.rsplit("::").next().unwrap_or(path)
}

pub(crate) fn name_of<F>(_job: &F) -> String {
    command_name(std::any::type_name::<F>()).to_string()
}

/// Logs a failed result once and hands it back unchanged.
pub(crate) fn record<T>(command: &str, result: Result<T, CommandError>) -> Result<T, CommandError> {
    if let Err(error) = &result {
        if let Some(line) = error_line(command, error) {
            crate::app_log::error(&line);
        }
    }
    result
}

/// Same as `record` for the synchronous commands, which know their own name.
pub(crate) fn logged<T, E: Into<CommandError>>(
    command: &str,
    result: Result<T, E>,
) -> Result<T, CommandError> {
    record(command, result.map_err(Into::into))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failure(code: &str, message: &str) -> CommandError {
        CommandError {
            code: code.into(),
            message: message.into(),
        }
    }

    #[test]
    fn a_failing_command_is_logged_with_name_and_code() {
        let line = error_line("play", &failure("write_failed", "Could not save.")).unwrap();
        assert!(line.contains("play"));
        assert!(line.contains("write_failed"));
    }

    #[test]
    fn benign_codes_are_not_logged() {
        for code in [
            "cancelled",
            "download_cancelled",
            "community_sign_in_denied",
        ] {
            assert!(error_line("x", &failure(code, "m")).is_none(), "{code}");
        }
    }

    #[test]
    fn secrets_in_the_message_are_redacted() {
        let secret = "abcdEFGH1234abcdEFGH1234abcdEFGH1234abcdEFGH1234";
        let line = error_line(
            "community_sign_in",
            &failure("community_sign_in_failed", &format!("token={secret}")),
        )
        .unwrap();
        assert!(!line.contains(secret));
    }

    #[test]
    fn the_command_name_comes_from_the_closure_type() {
        assert_eq!(
            command_name("motogp_launcher::commands::play::{{closure}}::{{closure}}"),
            "play"
        );
        assert_eq!(
            command_name("motogp_launcher::commands_install::install_catalog_entry::{{closure}}"),
            "install_catalog_entry"
        );
    }

    #[test]
    fn name_of_a_real_closure_names_its_enclosing_function() {
        fn some_command() -> String {
            let job = || 1;
            name_of(&job)
        }
        assert_eq!(some_command(), "some_command");
    }

    #[test]
    fn record_returns_the_result_unchanged() {
        let result: Result<u8, CommandError> = Err(failure("cancelled", "x"));
        assert_eq!(record("c", result).unwrap_err().code, "cancelled");
        assert_eq!(record("c", Ok::<u8, CommandError>(3)).unwrap(), 3);
    }
}
