//! Moderation routes for signed-in users: reporting a comment.
//!
//! # Write guard
//!
//! Every write route takes [`WritingUser`] instead of `AuthenticatedUser`. The
//! order of checks is: bearer session (401) -> general per-user limit (429) ->
//! ban (403 `banned`) -> the route's own limiter (comment writes, 429). A
//! banned user therefore always sees `banned`, never a rate-limit error.

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use community_application::AppError;
use serde::Deserialize;

use crate::comments::MAX_BODY_BYTES;
use crate::{ApiError, AppState, AuthenticatedUser};

/// A signed-in caller that is allowed to write (not banned).
#[derive(Debug, Clone)]
pub struct WritingUser(pub AuthenticatedUser);

impl FromRequestParts<AppState> for WritingUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthenticatedUser::from_request_parts(parts, state).await?;
        state.moderation.ensure_can_write(&auth.user.id).await?;
        Ok(Self(auth))
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mods/{mod_id}/comments/{comment_id}/reports", post(report))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
struct ReportRequest {
    reason: String,
    details: Option<String>,
}

/// `POST /mods/{mod_id}/comments/{comment_id}/reports`: idempotent per (user, comment).
async fn report(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path((mod_id, comment_id)): Path<(String, String)>,
    body: Result<Json<ReportRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    // Reports share the comment write bucket.
    app.comments.check_write(&auth.user)?;
    let Json(body) = body.map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))?;
    app.moderation
        .report(
            &auth.user,
            &mod_id,
            &comment_id,
            &body.reason,
            body.details.as_deref(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
