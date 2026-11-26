//! Mod comments: `/mods/{mod_id}/comments`.
//!
//! Reading is public (a valid bearer token only sets `own`). Writes need a
//! session and share their own per-user limit of 5 per minute.

use std::sync::Arc;

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::{DateTime, SecondsFormat, Utc};
use community_application::comments::{CommentWithAuthor, CommentsService};
use community_application::ports::Clock;
use community_application::AppError;
use community_domain::{User, UserId};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::{bearer_token, user_json};
use crate::{ApiError, AppState, AuthenticatedUser, RateLimitConfig, RateLimiter};

/// Comment writes (POST, PATCH, DELETE) allowed per user per minute.
pub const WRITES_PER_MINUTE: u32 = 5;
/// Largest accepted JSON request body.
pub const MAX_BODY_BYTES: usize = 16 * 1024;

/// What the comment handlers need besides the shared state.
#[derive(Clone)]
pub struct CommentsState {
    pub service: Arc<CommentsService>,
    /// Keyed by user id; separate from the general per-user limit.
    pub write_limiter: Arc<RateLimiter>,
}

impl CommentsState {
    pub fn new(service: Arc<CommentsService>, clock: Arc<dyn Clock>) -> Self {
        Self {
            service,
            write_limiter: Arc::new(RateLimiter::new(
                RateLimitConfig {
                    per_minute: WRITES_PER_MINUTE,
                },
                clock,
            )),
        }
    }

    fn check_write(&self, user: &User) -> Result<(), AppError> {
        self.write_limiter
            .check(user.id.as_str())
            .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mods/{mod_id}/comments", get(list).post(create))
        .route(
            "/mods/{mod_id}/comments/{comment_id}",
            patch(edit).delete(remove),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

fn rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// The public JSON shape of a comment. `viewer` is the signed-in caller, if any.
pub fn comment_json(item: &CommentWithAuthor, viewer: Option<&UserId>) -> Value {
    let author = user_json(&item.author);
    json!({
        "id": item.comment.id.as_str(),
        "body": item.comment.body.as_str(),
        "created_at": rfc3339(item.comment.created_at),
        "edited_at": item.comment.edited_at.map(rfc3339),
        "author": {
            "id": author["id"],
            "display_name": author["display_name"],
            "avatar_url": author["avatar_url"],
        },
        "own": viewer == Some(&item.comment.user_id),
    })
}

#[derive(Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct BodyRequest {
    body: String,
}

fn read_body(body: Result<Json<BodyRequest>, JsonRejection>) -> Result<String, AppError> {
    body.map(|Json(request)| request.body)
        .map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))
}

/// `GET /mods/{mod_id}/comments`: oldest first. A bad or expired token is ignored.
async fn list(
    State(app): State<AppState>,
    Path(mod_id): Path<String>,
    headers: HeaderMap,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let Query(query) =
        query.map_err(|_| AppError::BadRequest("The query string is invalid.".into()))?;
    let viewer = match bearer_token(&headers) {
        Some(token) => app.auth.authenticate(token).await.ok(),
        None => None,
    };
    let page = app
        .comments
        .service
        .list(&mod_id, query.cursor.as_deref(), query.limit)
        .await?;
    let viewer_id = viewer.as_ref().map(|user| &user.id);
    let comments: Vec<Value> = page
        .comments
        .iter()
        .map(|item| comment_json(item, viewer_id))
        .collect();
    Ok(Json(json!({
        "comments": comments,
        "next_cursor": page.next_cursor,
    })))
}

/// `POST /mods/{mod_id}/comments`.
async fn create(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    Path(mod_id): Path<String>,
    body: Result<Json<BodyRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    app.comments.check_write(&auth.user)?;
    let body = read_body(body)?;
    let created = app
        .comments
        .service
        .post(&auth.user, &mod_id, &body)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(comment_json(&created, Some(&auth.user.id))),
    ))
}

/// `PATCH /mods/{mod_id}/comments/{comment_id}`: author only (anyone else gets 404).
async fn edit(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    Path((mod_id, comment_id)): Path<(String, String)>,
    body: Result<Json<BodyRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    app.comments.check_write(&auth.user)?;
    let body = read_body(body)?;
    let edited = app
        .comments
        .service
        .edit(&auth.user, &mod_id, &comment_id, &body)
        .await?;
    Ok(Json(comment_json(&edited, Some(&auth.user.id))))
}

/// `DELETE /mods/{mod_id}/comments/{comment_id}`: author only (anyone else gets 404).
async fn remove(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    Path((mod_id, comment_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    app.comments.check_write(&auth.user)?;
    app.comments
        .service
        .delete(&auth.user, &mod_id, &comment_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use community_domain::comments::{Comment, CommentBody, CommentId, ModId};
    use community_domain::DiscordId;

    use super::*;

    fn item(edited: bool) -> CommentWithAuthor {
        let author = User {
            id: UserId("u1".into()),
            discord_id: DiscordId::parse("123").unwrap(),
            username: "rider".into(),
            global_name: Some("Rider One".into()),
            avatar_hash: Some("abc".into()),
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
        };
        CommentWithAuthor {
            comment: Comment {
                id: CommentId("c1".into()),
                mod_id: ModId::parse("mod-a").unwrap(),
                user_id: author.id.clone(),
                body: CommentBody::parse("Hello").unwrap(),
                created_at: DateTime::from_timestamp_millis(1_700_000_000_123).unwrap(),
                edited_at: edited
                    .then(|| DateTime::from_timestamp_millis(1_700_000_060_000).unwrap()),
            },
            author,
        }
    }

    #[test]
    fn comment_json_has_the_documented_shape_and_no_private_fields() {
        let value = comment_json(&item(false), None);
        assert_eq!(value["id"], "c1");
        assert_eq!(value["body"], "Hello");
        assert_eq!(value["created_at"], "2023-11-14T22:13:20.123Z");
        assert!(value["edited_at"].is_null());
        assert_eq!(value["author"]["id"], "u1");
        assert_eq!(value["author"]["display_name"], "Rider One");
        assert_eq!(
            value["author"]["avatar_url"],
            "https://cdn.discordapp.com/avatars/123/abc.png?size=64"
        );
        assert_eq!(value["own"], false);
        // The Discord id and username are not part of the comment author.
        let author = value["author"].as_object().unwrap();
        assert_eq!(author.len(), 3);
        assert!(value.as_object().unwrap().len() == 6);
    }

    #[test]
    fn comment_json_marks_edits_and_own_comments() {
        let value = comment_json(&item(true), Some(&UserId("u1".into())));
        assert_eq!(value["edited_at"], "2023-11-14T22:14:20.000Z");
        assert_eq!(value["own"], true);
        let other = comment_json(&item(true), Some(&UserId("u2".into())));
        assert_eq!(other["own"], false);
    }

    #[test]
    fn read_body_returns_the_text() {
        let ok = read_body(Ok(Json(BodyRequest { body: "hi".into() })));
        assert_eq!(ok.unwrap(), "hi");
    }
}
