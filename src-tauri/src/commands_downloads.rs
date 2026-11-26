//! Anonymous download counts (issue #120). Nothing here ever fails the caller:
//! the WebView gets `null` (or nothing) when the server cannot be reached, and
//! the blocking network work stays off the async runtime thread.
use crate::{AppState, CommandError};
use motogp_application::{DailyDownloads, ModDownloads};
use serde::Serialize;
use tauri::State;

/// Download totals of one mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDownloadsDto {
    pub id: String,
    pub total: u64,
    pub last30_days: u64,
}

impl From<ModDownloads> for ModDownloadsDto {
    fn from(value: ModDownloads) -> Self {
        Self {
            id: value.id,
            total: value.total,
            last30_days: value.last_30_days,
        }
    }
}

/// One UTC day (`YYYY-MM-DD`) of downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyDownloadsDto {
    pub date: String,
    pub count: u64,
}

impl From<DailyDownloads> for DailyDownloadsDto {
    fn from(value: DailyDownloads) -> Self {
        Self {
            date: value.date,
            count: value.count,
        }
    }
}

/// Runs a blocking read; a task that did not finish is just "nothing".
async fn quietly<T, F>(state: State<'_, AppState>, job: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Option<T> + Send + 'static,
{
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || job(&state))
        .await
        .ok()
        .flatten()
}

/// Counts one successful catalog install (once per mod, install and UTC day).
/// Never errors; a failure is silent.
#[tauri::command]
pub async fn report_mod_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), CommandError> {
    quietly(state, move |state| {
        state.downloads.report(&id);
        Some(())
    })
    .await;
    Ok(())
}

/// Totals of every mod with downloads, or `null` when the server is out of reach.
#[tauri::command]
pub async fn get_mod_downloads(
    state: State<'_, AppState>,
) -> Result<Option<Vec<ModDownloadsDto>>, CommandError> {
    Ok(quietly(state, |state| {
        state
            .downloads
            .totals()
            .map(|totals| totals.into_iter().map(Into::into).collect())
    })
    .await)
}

/// The 30 UTC days of one mod, oldest first, or `null`.
#[tauri::command]
pub async fn get_mod_download_daily(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Vec<DailyDownloadsDto>>, CommandError> {
    Ok(quietly(state, move |state| {
        state
            .downloads
            .daily(&id)
            .map(|days| days.into_iter().map(Into::into).collect())
    })
    .await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_totals_dto_is_camel_case() {
        let dto: ModDownloadsDto = ModDownloads {
            id: "a".into(),
            total: 101,
            last_30_days: 7,
        }
        .into();
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            json!({ "id": "a", "total": 101, "last30Days": 7 })
        );
    }

    #[test]
    fn the_daily_dto_has_date_and_count() {
        let dto: DailyDownloadsDto = DailyDownloads {
            date: "2026-10-28".into(),
            count: 3,
        }
        .into();
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            json!({ "date": "2026-10-28", "count": 3 })
        );
    }
}
