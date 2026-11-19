//! Thin Tauri commands: convert arguments and DTOs, call one use case and map
//! the error. No business rules live here. JS passes camelCase arguments
//! (`modIds`, `profileId`); results use snake_case DTO fields.
//!
//! Reads are plain synchronous commands. Everything that writes (imports,
//! deploys, profiles, settings) is `async`: the blocking use case runs on the
//! blocking pool, never on the main thread, and mutations are serialized
//! through `AppState::write_lock` so they cannot lose each other's updates.
use crate::dto::{ConfigDto, GameInfoDto, ModDto, ProfileDto, RecoveryStatusDto};
use crate::{AppState, CommandError};
use motogp_application::{import_mods as import_batch, remove_mod as remove_everywhere};
use std::path::{Path, PathBuf};
use tauri::{Builder, Runtime, State};

type CommandResult<T> = Result<T, CommandError>;

/// Runs a blocking use case off the caller's thread, one mutation at a time.
async fn mutate<T, F>(state: State<'_, AppState>, job: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> CommandResult<T> + Send + 'static,
{
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _serialized = state.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        job(&state)
    })
    .await
    .map_err(|error| CommandError {
        code: "task_failed".to_string(),
        message: format!("the operation did not finish: {error}"),
    })?
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> CommandResult<ConfigDto> {
    Ok(state.settings.config()?.into())
}

#[tauri::command]
pub async fn detect_games(state: State<'_, AppState>) -> CommandResult<Vec<GameInfoDto>> {
    mutate(state, |state| {
        Ok(state
            .settings
            .detect_games()?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn set_game_dir(state: State<'_, AppState>, path: String) -> CommandResult<GameInfoDto> {
    mutate(state, move |state| {
        Ok(state.settings.set_game_dir(Path::new(&path))?.into())
    })
    .await
}

#[tauri::command]
pub fn get_game(state: State<'_, AppState>) -> CommandResult<Option<GameInfoDto>> {
    Ok(state.settings.get_game()?.map(Into::into))
}

#[tauri::command]
pub fn list_mods(state: State<'_, AppState>) -> CommandResult<Vec<ModDto>> {
    Ok(state.library.list()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn import_mods(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CommandResult<Vec<ModDto>> {
    mutate(state, move |state| {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        Ok(import_batch(&state.library, &paths)?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn remove_mod(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(remove_everywhere(&state.library, &state.profiles, &id)?)
    })
    .await
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> CommandResult<Vec<ProfileDto>> {
    Ok(state.profiles.list()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn create_profile(state: State<'_, AppState>, name: String) -> CommandResult<ProfileDto> {
    mutate(state, move |state| Ok(state.profiles.create(&name)?.into())).await
}

#[tauri::command]
pub async fn rename_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<ProfileDto> {
    mutate(state, move |state| {
        Ok(state.profiles.rename(&id, &name)?.into())
    })
    .await
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(state.profiles.delete(&id, &state.config)?)
    })
    .await
}

#[tauri::command]
pub async fn set_profile_mods(
    state: State<'_, AppState>,
    id: String,
    mod_ids: Vec<String>,
) -> CommandResult<ProfileDto> {
    mutate(state, move |state| {
        Ok(state.profiles.set_mods(&id, mod_ids)?.into())
    })
    .await
}

#[tauri::command]
pub async fn set_active_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    mutate(state, move |state| {
        Ok(state.profiles.set_active(&id, &state.config)?)
    })
    .await
}

#[tauri::command]
pub async fn play(state: State<'_, AppState>, profile_id: String) -> CommandResult<()> {
    mutate(state, move |state| Ok(state.play.play(&profile_id)?)).await
}

/// The startup recovery result. The frontend reads it after subscribing to
/// the session events, so a failure that happened before it listened is kept.
#[tauri::command]
pub fn recovery_status(state: State<'_, AppState>) -> RecoveryStatusDto {
    state.play.recovery_status().into()
}

/// Registers every command on the builder; shared by the real app and the
/// contract tests.
pub fn configure<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        get_config,
        detect_games,
        set_game_dir,
        get_game,
        list_mods,
        import_mods,
        remove_mod,
        list_profiles,
        create_profile,
        rename_profile,
        delete_profile,
        set_profile_mods,
        set_active_profile,
        play,
        recovery_status
    ])
}
