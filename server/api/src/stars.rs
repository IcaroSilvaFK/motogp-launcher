//! Stars: `PUT`/`DELETE /mods/{id}/star` and `GET /mods/stats`.

use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::routing::{get, put};
use axum::{Json, Router};
use community_application::{AppError, ModStats};
use community_domain::User;
use serde::Serialize;

use crate::auth::bearer_token;
use crate::{ApiError, AppState, WritingUser};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mods/stats", get(stats))
        .route("/mods/{id}/star", put(star).delete(unstar))
}

/// A caller that may or may not be signed in. A missing, unknown, expired or
/// revoked token is an anonymous caller (never `401`), so public counts keep
/// working when a session goes stale. A valid session also pays the per-user
/// rate limit.
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<User>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(bearer) = bearer_token(&parts.headers) else {
            return Ok(Self(None));
        };
        match state.auth.authenticate(bearer).await {
            Ok(user) => {
                state.check_user_limit(&user.id)?;
                Ok(Self(Some(user)))
            }
            Err(AppError::Unauthorized) => Ok(Self(None)),
            Err(other) => Err(other.into()),
        }
    }
}

/// Public JSON shape of [`ModStats`]; later issues add fields next to these.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModStatsDto {
    pub id: String,
    pub stars: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starred_by_me: Option<bool>,
}

impl From<ModStats> for ModStatsDto {
    fn from(stats: ModStats) -> Self {
        Self {
            id: stats.id,
            stars: stats.stars,
            starred_by_me: stats.starred_by_me,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub mods: Vec<ModStatsDto>,
}

/// `PUT /mods/{id}/star`
async fn star(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
) -> Result<Json<ModStatsDto>, ApiError> {
    Ok(Json(app.stars.star(&auth.user.id, &id).await?.into()))
}

/// `DELETE /mods/{id}/star`
async fn unstar(
    State(app): State<AppState>,
    WritingUser(auth): WritingUser,
    Path(id): Path<String>,
) -> Result<Json<ModStatsDto>, ApiError> {
    Ok(Json(app.stars.unstar(&auth.user.id, &id).await?.into()))
}

/// `GET /mods/stats`: mods with at least one star; `starredByMe` only with a valid session.
async fn stats(
    State(app): State<AppState>,
    OptionalUser(viewer): OptionalUser,
) -> Result<Json<StatsResponse>, ApiError> {
    let mods = app
        .stars
        .stats(viewer.as_ref().map(|user| &user.id))
        .await?;
    Ok(Json(StatsResponse {
        mods: mods.into_iter().map(ModStatsDto::from).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_is_camel_case_and_skips_the_missing_flag() {
        let anonymous = ModStatsDto::from(ModStats {
            id: "m".into(),
            stars: 3,
            starred_by_me: None,
        });
        assert_eq!(
            serde_json::to_string(&anonymous).unwrap(),
            r#"{"id":"m","stars":3}"#
        );
        let mine = ModStatsDto::from(ModStats {
            id: "m".into(),
            stars: 3,
            starred_by_me: Some(true),
        });
        assert_eq!(
            serde_json::to_string(&mine).unwrap(),
            r#"{"id":"m","stars":3,"starredByMe":true}"#
        );
    }
}
