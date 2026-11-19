//! Thin Tauri commands: convert arguments and DTOs, call one use case and map
//! the error. No business rules live here. JS passes camelCase arguments
//! (`modIds`, `profileId`); results use snake_case DTO fields.
use crate::dto::{ConfigDto, GameInfoDto, ModDto, ProfileDto};
use crate::{AppState, CommandError};
use motogp_application::{import_mods as import_batch, remove_mod as remove_everywhere};
use std::path::{Path, PathBuf};
use tauri::{Builder, Runtime, State};

type CommandResult<T> = Result<T, CommandError>;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> CommandResult<ConfigDto> {
    Ok(state.settings.config()?.into())
}

#[tauri::command]
pub fn detect_games(state: State<'_, AppState>) -> CommandResult<Vec<GameInfoDto>> {
    Ok(state
        .settings
        .detect_games()?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub fn set_game_dir(state: State<'_, AppState>, path: String) -> CommandResult<GameInfoDto> {
    Ok(state.settings.set_game_dir(Path::new(&path))?.into())
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
pub fn import_mods(state: State<'_, AppState>, paths: Vec<String>) -> CommandResult<Vec<ModDto>> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    Ok(import_batch(&state.library, &paths)?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub fn remove_mod(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    Ok(remove_everywhere(&state.library, &state.profiles, &id)?)
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> CommandResult<Vec<ProfileDto>> {
    Ok(state
        .profiles
        .list()?
        .into_iter()
        .map(Into::into)
        .collect())
}

#[tauri::command]
pub fn create_profile(state: State<'_, AppState>, name: String) -> CommandResult<ProfileDto> {
    Ok(state.profiles.create(&name)?.into())
}

#[tauri::command]
pub fn rename_profile(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> CommandResult<ProfileDto> {
    Ok(state.profiles.rename(&id, &name)?.into())
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    Ok(state.profiles.delete(&id, &state.config)?)
}

#[tauri::command]
pub fn set_profile_mods(
    state: State<'_, AppState>,
    id: String,
    mod_ids: Vec<String>,
) -> CommandResult<ProfileDto> {
    Ok(state.profiles.set_mods(&id, mod_ids)?.into())
}

#[tauri::command]
pub fn set_active_profile(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    Ok(state.profiles.set_active(&id, &state.config)?)
}

#[tauri::command]
pub fn play(state: State<'_, AppState>, profile_id: String) -> CommandResult<()> {
    Ok(state.play.play(&profile_id)?)
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
        play
    ])
}
