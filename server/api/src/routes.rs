//! Route handlers. Later issues add their routers in `extra_routes`.

use axum::extract::State;
use axum::Json;
use axum::Router;
use community_application::AppError;
use serde_json::{json, Value};

use crate::{ApiError, AppState};

/// Routes added by feature issues. Auth is registered here; comments, stars... follow.
pub fn extra_routes() -> Router<AppState> {
    Router::new()
        .merge(crate::auth::routes())
        .merge(crate::admin_web::routes())
        .merge(crate::stars::routes())
        .merge(crate::comments::routes())
        .merge(crate::moderation::routes())
        .merge(crate::downloads::routes())
        .merge(crate::mods::routes())
        .merge(crate::uploads::routes())
}

/// `GET /health`: 200 when the database answers.
pub async fn health(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    state.health.check().await?;
    Ok(Json(json!({ "status": "ok" })))
}

/// Fallback for unknown routes.
pub async fn not_found() -> ApiError {
    ApiError(AppError::NotFound)
}
