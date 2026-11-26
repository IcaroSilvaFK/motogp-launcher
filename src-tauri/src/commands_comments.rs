//! Thin Tauri commands of the mod comments (#118). Network I/O runs on the
//! blocking pool and never under `write_lock`. The session token stays in the
//! application layer: none of these commands accepts or returns it.
use crate::commands::read_blocking;
use crate::dto_comments::{CommentsPageDto, ModCommentDto};
use crate::{AppState, CommandError};
use motogp_application::CommentsError;
use motogp_domain::ReportReason;
use tauri::State;

type CommandResult<T> = Result<T, CommandError>;

/// One page of comments, oldest first. Works signed out and offline-safe: an
/// unreachable server is the `community_offline` error.
#[tauri::command]
pub async fn community_comments_list(
    state: State<'_, AppState>,
    mod_id: String,
    cursor: Option<String>,
) -> CommandResult<CommentsPageDto> {
    read_blocking(state, move |state| {
        Ok(state.comments.list(&mod_id, cursor.as_deref())?.into())
    })
    .await
}

#[tauri::command]
pub async fn community_comment_post(
    state: State<'_, AppState>,
    mod_id: String,
    body: String,
) -> CommandResult<ModCommentDto> {
    read_blocking(state, move |state| {
        Ok(state.comments.post(&mod_id, &body)?.into())
    })
    .await
}

#[tauri::command]
pub async fn community_comment_edit(
    state: State<'_, AppState>,
    mod_id: String,
    comment_id: String,
    body: String,
) -> CommandResult<ModCommentDto> {
    read_blocking(state, move |state| {
        Ok(state.comments.edit(&mod_id, &comment_id, &body)?.into())
    })
    .await
}

#[tauri::command]
pub async fn community_comment_delete(
    state: State<'_, AppState>,
    mod_id: String,
    comment_id: String,
) -> CommandResult<()> {
    read_blocking(state, move |state| {
        Ok(state.comments.delete(&mod_id, &comment_id)?)
    })
    .await
}

/// Reports another member's comment. `reason` is `spam`, `abuse`, `off_topic`
/// or `other`; `details` is optional free text (500 characters at most).
#[tauri::command]
pub async fn community_report_comment(
    state: State<'_, AppState>,
    mod_id: String,
    comment_id: String,
    reason: String,
    details: Option<String>,
) -> CommandResult<()> {
    let reason =
        ReportReason::parse(&reason).ok_or_else(|| CommandError::from(CommentsError::Invalid))?;
    read_blocking(state, move |state| {
        Ok(state
            .comments
            .report(&mod_id, &comment_id, reason, details.as_deref())?)
    })
    .await
}
