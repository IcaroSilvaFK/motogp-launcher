//! Thin Tauri commands of the catalog install (#74). The download runs on the
//! blocking pool WITHOUT `write_lock` (it can take minutes); only the import
//! runs under it, like every other library mutation.
use crate::commands::{mutate, read_blocking};
use crate::dto::ModDto;
use crate::{AppState, CommandError};
use motogp_application::{
    CancelFlag, CatalogInstallError, ConfigRepository, InstallOutcome, InstallPhase,
    InstallProgress,
};
use motogp_domain::VANILLA_ID;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State};

/// Event name of install progress.
pub const CATALOG_INSTALL_PROGRESS_EVENT: &str = "catalog-install-progress";

/// Payload of [`CATALOG_INSTALL_PROGRESS_EVENT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInstallProgressDto {
    pub id: String,
    /// `downloading`, `verifying` or `importing`.
    pub phase: &'static str,
    pub received: u64,
    pub total: u64,
}

impl CatalogInstallProgressDto {
    fn from_progress(id: &str, progress: InstallProgress) -> Self {
        Self {
            id: id.to_string(),
            phase: match progress.phase {
                InstallPhase::Downloading => "downloading",
                InstallPhase::Verifying => "verifying",
            },
            received: progress.received,
            total: progress.total,
        }
    }

    fn importing(id: &str, size: u64) -> Self {
        Self {
            id: id.to_string(),
            phase: "importing",
            received: size,
            total: size,
        }
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, payload: CatalogInstallProgressDto) {
    if let Err(error) = app.emit(CATALOG_INSTALL_PROGRESS_EVENT, payload) {
        eprintln!("could not emit the install progress: {error}");
    }
}

/// Result of [`install_catalog_entry`] (#129): the imported mods and whether
/// the first one was enabled in the active profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInstallResultDto {
    pub mods: Vec<ModDto>,
    pub mod_id: String,
    pub enabled: bool,
    /// Name of the enabled mod that blocked enabling, when there was a conflict.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_with: Option<String>,
}

impl From<InstallOutcome> for CatalogInstallResultDto {
    fn from(outcome: InstallOutcome) -> Self {
        Self {
            mods: outcome.mods.into_iter().map(Into::into).collect(),
            mod_id: outcome.mod_id,
            enabled: outcome.enabled,
            conflict_with: outcome.conflict_with,
        }
    }
}

/// Removes the cancel flag of an install when it ends, however it ends.
struct CancelRegistration<'a> {
    state: &'a AppState,
    id: String,
}

impl<'a> CancelRegistration<'a> {
    /// `None` when the entry already has an install in flight.
    fn register(state: &'a AppState, id: &str, flag: &CancelFlag) -> Option<Self> {
        let mut flags = state
            .install_cancels
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if flags.contains_key(id) {
            return None;
        }
        flags.insert(id.to_string(), flag.clone());
        Some(Self {
            state,
            id: id.to_string(),
        })
    }
}

impl Drop for CancelRegistration<'_> {
    fn drop(&mut self) {
        self.state
            .install_cancels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.id);
    }
}

/// Downloads, verifies and imports the catalog entry `id`. The frontend only
/// names the entry: the URL always comes from the catalog. With
/// `enable_after_install` the new mod is also enabled in the active profile
/// (unless it conflicts with an enabled mod).
#[tauri::command]
pub async fn install_catalog_entry<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
    enable_after_install: Option<bool>,
) -> Result<CatalogInstallResultDto, CommandError> {
    let cancel = CancelFlag::new();
    let _registration = CancelRegistration::register(state.inner(), &id, &cancel)
        .ok_or_else(|| CatalogInstallError::InstallInProgress { id: id.clone() })?;

    let prepare_app = app.clone();
    let prepare_id = id.clone();
    let staged = read_blocking(state.clone(), move |state| {
        let mut report = |progress| {
            emit(
                &prepare_app,
                CatalogInstallProgressDto::from_progress(&prepare_id, progress),
            )
        };
        Ok(state.installer.prepare(
            &state.catalog,
            &state.profiles,
            &prepare_id,
            &mut report,
            &cancel,
        )?)
    })
    .await?;

    mutate(state, move |state| {
        emit(
            &app,
            CatalogInstallProgressDto::importing(&id, staged.size()),
        );
        let target = match enable_after_install {
            Some(true) => Some(
                state
                    .config
                    .load()
                    .ok()
                    .and_then(|config| config.active_profile)
                    .unwrap_or_else(|| VANILLA_ID.to_string()),
            ),
            _ => None,
        };
        let outcome =
            state
                .installer
                .install(&state.library, &state.profiles, staged, target.as_deref())?;
        Ok(outcome.into())
    })
    .await
}

/// Asks the install of `id` to stop. A no-op when none is running.
#[tauri::command]
pub fn cancel_catalog_install(state: State<'_, AppState>, id: String) {
    let flags = state
        .install_cancels
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(flag) = flags.get(&id) {
        flag.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_payload_is_camel_case_with_a_lowercase_phase() {
        let dto = CatalogInstallProgressDto::from_progress(
            "rebalanced-ai",
            InstallProgress {
                phase: InstallPhase::Downloading,
                received: 5,
                total: 10,
            },
        );
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            serde_json::json!({
                "id": "rebalanced-ai",
                "phase": "downloading",
                "received": 5,
                "total": 10
            })
        );
    }

    #[test]
    fn phases_map_to_their_wire_names() {
        let verifying = CatalogInstallProgressDto::from_progress(
            "a",
            InstallProgress {
                phase: InstallPhase::Verifying,
                received: 1,
                total: 1,
            },
        );
        assert_eq!(verifying.phase, "verifying");
        let importing = CatalogInstallProgressDto::importing("a", 7);
        assert_eq!(
            (importing.phase, importing.received, importing.total),
            ("importing", 7, 7)
        );
    }
}
