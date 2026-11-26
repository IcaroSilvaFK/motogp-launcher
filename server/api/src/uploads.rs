//! Resumable mod uploads: create, chunks, resume, screenshots, complete, abort.

use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, JsonRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{Method, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::SecondsFormat;
use community_application::AppError;
use community_domain::mods::{RawMetadata, CHUNK_SIZE, MAX_SCREENSHOT_BYTES};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::mods::version_json;
use crate::{ApiError, AppState, AuthenticatedUser, WritingUser};

pub fn routes() -> Router<AppState> {
    // The default 2 MB body limit is raised only where bytes are expected.
    Router::new()
        .route("/mods/uploads", post(create))
        .route(
            "/mods/uploads/{id}/chunks",
            put(chunk).layer(DefaultBodyLimit::max(CHUNK_SIZE as usize)),
        )
        .route(
            "/mods/uploads/{id}/screenshots",
            post(screenshot).layer(DefaultBodyLimit::max(MAX_SCREENSHOT_BYTES as usize)),
        )
        .route("/mods/uploads/{id}", get(status).delete(abort))
        .route("/mods/uploads/{id}/complete", post(complete))
}

/// `PUT /mods/uploads/{id}/chunks`: many per upload, so the per-IP limit skips it
/// (the per-user limit still applies).
pub fn is_chunk_request(method: &Method, path: &str) -> bool {
    method == Method::PUT
        && path
            .strip_prefix("/mods/uploads/")
            .and_then(|rest| rest.strip_suffix("/chunks"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBody {
    mod_id: String,
    title: String,
    description: String,
    long_description: Option<String>,
    category: String,
    game: String,
    version: String,
    file_name: String,
    size: u64,
    sha256: Option<String>,
    #[serde(default)]
    ownership_confirmed: bool,
}

/// `POST /mods/uploads`: starts an upload; refused here when too large, over quota or rate-limited.
async fn create(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    body: Result<Json<CreateBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Json(body) = body.map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))?;
    let session = app
        .mods
        .service
        .create_upload(
            &auth.user,
            RawMetadata {
                mod_id: body.mod_id,
                title: body.title,
                description: body.description,
                long_description: body.long_description,
                category: body.category,
                game: body.game,
                version: body.version,
                file_name: body.file_name,
                size: body.size,
                sha256: body.sha256,
                ownership_confirmed: body.ownership_confirmed,
            },
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "uploadId": session.upload_id,
            "chunkSize": session.chunk_size,
            "received": session.received,
            "expiresAt": session.expires_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        })),
    ))
}

#[derive(Deserialize)]
struct ChunkQuery {
    offset: u64,
}

fn body_error(rejection: &BytesRejection) -> AppError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::FileTooLarge
    } else {
        AppError::BadRequest("The request body could not be read.".into())
    }
}

/// `PUT /mods/uploads/{id}/chunks?offset=N`: raw bytes, `offset` must equal the bytes received.
async fn chunk(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
    query: Result<Query<ChunkQuery>, QueryRejection>,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<Value>, ApiError> {
    let Query(query) =
        query.map_err(|_| AppError::BadRequest("The offset is missing or invalid.".into()))?;
    let bytes = body.map_err(|rejection| body_error(&rejection))?;
    let received = app
        .mods
        .service
        .write_chunk(&auth.user, &id, query.offset, &bytes)
        .await?;
    Ok(Json(json!({ "received": received })))
}

/// `GET /mods/uploads/{id}`: resume information.
async fn status(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let status = app.mods.service.upload_status(&auth.user, &id).await?;
    Ok(Json(json!({
        "received": status.received,
        "size": status.size,
        "state": status.state.as_str(),
    })))
}

/// `POST /mods/uploads/{id}/screenshots`: a raw PNG, JPEG or WebP image.
async fn screenshot(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
    body: Result<Bytes, BytesRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let bytes = body.map_err(|rejection| body_error(&rejection))?;
    let index = app
        .mods
        .service
        .add_screenshot(&auth.user, &id, &bytes)
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "index": index }))))
}

/// `POST /mods/uploads/{id}/complete`: verifies the bytes and creates the pending version.
async fn complete(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let version = app.mods.service.complete(&auth.user, &id).await?;
    Ok((StatusCode::CREATED, Json(version_json(&version))))
}

/// `DELETE /mods/uploads/{id}`: aborts and deletes the bytes.
async fn abort(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    app.mods.service.abort(&auth.user, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_chunk_puts_skip_the_ip_limit() {
        assert!(is_chunk_request(&Method::PUT, "/mods/uploads/abc/chunks"));
        assert!(!is_chunk_request(&Method::POST, "/mods/uploads/abc/chunks"));
        assert!(!is_chunk_request(&Method::PUT, "/mods/uploads//chunks"));
        assert!(!is_chunk_request(&Method::PUT, "/mods/uploads/a/b/chunks"));
        assert!(!is_chunk_request(&Method::PUT, "/mods/uploads/abc"));
        assert!(!is_chunk_request(&Method::PUT, "/other"));
    }
}
