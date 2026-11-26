//! Authentication: `/auth/*` and `/me` handlers plus the reusable
//! [`AuthenticatedUser`] extractor.
//!
//! # Protecting a route
//!
//! Add `AuthenticatedUser` to the handler arguments. The extractor reads
//! `Authorization: Bearer <token>`, resolves the session, applies the per-user
//! rate limit and rejects with the standard `401 unauthorized` JSON error:
//!
//! ```ignore
//! async fn my_route(auth: AuthenticatedUser) -> Result<Json<Value>, ApiError> {
//!     Ok(Json(json!({ "hello": auth.user.display_name() })))
//! }
//! ```

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequestParts, Query, State};
use axum::http::header::{AUTHORIZATION, LOCATION};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use community_application::auth::{CallbackOutcome, CallbackParams};
use community_application::AppError;
use community_domain::{SessionToken, User};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ApiError, AppState};

/// A signed-in caller. Use it as a handler argument to require a session.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user: User,
    /// The bearer token that was presented (needed for logout).
    pub token: SessionToken,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let bearer = bearer_token(&parts.headers).ok_or(AppError::Unauthorized)?;
        let user = state.auth.authenticate(bearer).await?;
        state.check_user_limit(&user.id)?;
        Ok(Self {
            user,
            token: SessionToken::new(bearer),
        })
    }
}

/// Extracts the token of an `Authorization: Bearer <token>` header.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// The public JSON shape of a user.
pub fn user_json(user: &User) -> Value {
    let avatar_url = user.avatar_hash.as_ref().map(|hash| {
        format!(
            "https://cdn.discordapp.com/avatars/{}/{hash}.png?size=64",
            user.discord_id.as_str()
        )
    });
    json!({
        "id": user.id.as_str(),
        "discord_id": user.discord_id.as_str(),
        "username": user.username,
        "display_name": user.display_name(),
        "avatar_url": avatar_url,
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/discord/start", get(start))
        .route("/auth/discord/callback", get(callback))
        .route("/auth/token", post(token))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
}

fn found(location: &str) -> Result<Response, ApiError> {
    let value = location
        .parse()
        .map_err(|_| AppError::Internal("redirect is not a valid header value".into()))?;
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(LOCATION, value);
    Ok(response)
}

#[derive(Deserialize)]
pub struct StartQuery {
    redirect: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
}

/// `GET /auth/discord/start`: 302 to Discord's consent screen.
async fn start(
    State(app): State<AppState>,
    query: Result<Query<StartQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) =
        query.map_err(|_| AppError::BadRequest("The query string is invalid.".into()))?;
    let redirect = query.redirect.ok_or(AppError::InvalidRedirect)?;
    let state = query
        .state
        .ok_or_else(|| AppError::BadRequest("The state is missing.".into()))?;
    let challenge = query
        .code_challenge
        .ok_or_else(|| AppError::BadRequest("The code challenge is missing.".into()))?;
    let url = app.auth.start(&redirect, &state, &challenge).await?;
    found(&url)
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// `GET /auth/discord/callback`: 302 to the app's loopback with the one-time
/// code, or (for an admin web sign-in) the admin session page.
// Both arms are axum responses; boxing the error would only add noise.
#[allow(clippy::result_large_err)]
async fn callback(
    State(app): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<CallbackQuery>, QueryRejection>,
) -> Result<Response, Response> {
    // A browser that started an admin sign-in carries the `admin_login` cookie:
    // show its errors as a page instead of JSON.
    let admin_cookie = crate::admin_web::cookie_value(&headers, "admin_login");
    let fail = |error: AppError| -> Response {
        if admin_cookie.is_some() {
            crate::admin_web::error_page(&error)
        } else {
            ApiError(error).into_response()
        }
    };
    let Query(query) =
        query.map_err(|_| fail(AppError::BadRequest("The query string is invalid.".into())))?;
    let outcome = app
        .auth
        .finish_callback(
            CallbackParams {
                code: query.code,
                error: query.error,
                state: query.state,
            },
            admin_cookie,
        )
        .await
        .map_err(fail)?;
    match outcome {
        CallbackOutcome::App { location } => found(&location).map_err(IntoResponse::into_response),
        CallbackOutcome::Admin { user } => crate::admin_web::finish_login(&app, user)
            .await
            .map_err(fail),
    }
}

#[derive(Deserialize)]
pub struct TokenRequest {
    code: String,
    code_verifier: String,
}

/// `POST /auth/token`: trades the one-time code for a session token.
async fn token(
    State(app): State<AppState>,
    body: Result<Json<TokenRequest>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let Json(body) = body.map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))?;
    let issued = app.auth.exchange(&body.code, &body.code_verifier).await?;
    Ok(Json(json!({
        "token": issued.token.expose(),
        "expires_at": issued.expires_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "user": user_json(&issued.user),
    })))
}

/// `GET /me`: the signed-in user.
async fn me(auth: AuthenticatedUser) -> Json<Value> {
    Json(user_json(&auth.user))
}

/// `POST /auth/logout`: revokes the presented session.
async fn logout(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<StatusCode, ApiError> {
    app.auth.logout(auth.token.expose()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;
    use community_domain::{DiscordId, UserId};

    use super::*;

    fn headers(value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn bearer_token_parsing() {
        assert_eq!(bearer_token(&headers("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(&headers("bearer abc")), Some("abc"));
        assert_eq!(bearer_token(&headers("Basic abc")), None);
        assert_eq!(bearer_token(&headers("Bearer ")), None);
        assert_eq!(bearer_token(&headers("Bearer")), None);
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn user_json_builds_the_avatar_url_or_null() {
        let mut user = User {
            id: UserId("u1".into()),
            discord_id: DiscordId::parse("123").unwrap(),
            username: "rider".into(),
            global_name: Some("Rider One".into()),
            avatar_hash: Some("abc".into()),
            created_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
        };
        let value = user_json(&user);
        assert_eq!(value["display_name"], "Rider One");
        assert_eq!(value["username"], "rider");
        assert_eq!(
            value["avatar_url"],
            "https://cdn.discordapp.com/avatars/123/abc.png?size=64"
        );
        user.avatar_hash = None;
        assert!(user_json(&user)["avatar_url"].is_null());
    }
}
