//! Thin Tauri commands of the mod stars (#119). Blocking network I/O runs on
//! the blocking pool and never under `write_lock`. The session token stays in
//! the service: it never crosses IPC.
use crate::commands::read_blocking;
use crate::dto_stars::{ModStatsDto, ModStatsListDto};
use crate::{AppState, CommandError};
use tauri::State;

/// Stars of every mod that has any. Includes `starredByMe` when signed in. An
/// unreachable server is the `community_offline` error.
#[tauri::command]
pub async fn community_mod_stats(
    state: State<'_, AppState>,
) -> Result<ModStatsListDto, CommandError> {
    read_blocking(state, |state| Ok(state.stars.stats()?.into())).await
}

/// Stars a mod for the signed-in user (`community_signed_out` otherwise).
#[tauri::command]
pub async fn community_star(
    state: State<'_, AppState>,
    id: String,
) -> Result<ModStatsDto, CommandError> {
    read_blocking(state, move |state| Ok(state.stars.star(&id)?.into())).await
}

/// Removes the signed-in user's star.
#[tauri::command]
pub async fn community_unstar(
    state: State<'_, AppState>,
    id: String,
) -> Result<ModStatsDto, CommandError> {
    read_blocking(state, move |state| Ok(state.stars.unstar(&id)?.into())).await
}
