//! Diagnostics commands (issue #148): the data for the "Copy diagnostics"
//! report and the "Open logs folder" action. Neither takes a path.
use crate::commands::read_blocking;
use crate::{AppState, CommandError};
use motogp_application::DiagnosticsSnapshot;
use serde::Serialize;
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

/// What the backend contributes to a diagnostics report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsDto {
    pub os: String,
    /// The most recent log lines, oldest first, already redacted.
    pub log_lines: Vec<String>,
}

impl From<DiagnosticsSnapshot> for DiagnosticsDto {
    fn from(snapshot: DiagnosticsSnapshot) -> Self {
        Self {
            os: snapshot.os,
            log_lines: snapshot.log_lines,
        }
    }
}

/// The OS and the last log lines, secrets removed.
#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> CommandResult<DiagnosticsDto> {
    read_blocking(state, |state| Ok(state.diagnostics.snapshot().into())).await
}

/// Shows the folder that holds the log files in the file manager.
#[tauri::command]
pub async fn open_logs_folder(state: State<'_, AppState>) -> CommandResult<()> {
    read_blocking(state, |state| Ok(state.diagnostics.open_logs_folder()?)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dto_is_camel_case() {
        let dto = DiagnosticsDto::from(DiagnosticsSnapshot {
            os: "Windows 11".into(),
            log_lines: vec!["a".into()],
        });
        let json = serde_json::to_value(dto).expect("the dto serializes");
        assert_eq!(
            json,
            serde_json::json!({ "os": "Windows 11", "logLines": ["a"] })
        );
    }
}
