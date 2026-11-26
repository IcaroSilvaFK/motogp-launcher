//! Community server HTTP interface: router, error format, rate limiting.

pub mod auth;
pub mod comments;
pub mod downloads;
pub mod error;
pub mod rate_limit;
pub mod routes;
pub mod stars;
pub mod testing;

use std::sync::Arc;

use axum::extract::Request;
use axum::http::Uri;
use axum::middleware;
use axum::routing::get;
use axum::Router;
use community_application::ports::{Clock, HealthProbe};
use community_application::AppError;
use community_application::AuthService;
use community_application::StarService;
use community_domain::UserId;
use tower_http::trace::TraceLayer;

pub use auth::AuthenticatedUser;
pub use error::ApiError;
pub use rate_limit::{RateLimitConfig, RateLimiter};

/// Shared state of every handler. Later issues add their ports here.
#[derive(Clone)]
pub struct AppState {
    pub clock: Arc<dyn Clock>,
    pub health: Arc<dyn HealthProbe>,
    /// Sign-in use cases and session lookup (used by the `AuthenticatedUser` extractor).
    pub auth: Arc<AuthService>,
    /// Star use cases.
    pub stars: Arc<StarService>,
    pub limits: RateLimits,
    /// Mod comments use cases and their write limiter.
    pub comments: comments::CommentsState,
    /// Trust the right-most `X-Forwarded-For` (only behind our own proxy).
    pub trust_proxy_headers: bool,
    /// Download counts: service and its report limiter.
    pub downloads: downloads::DownloadsState,
}

/// One limiter per route group.
#[derive(Clone)]
pub struct RateLimits {
    /// Every route, keyed by client IP.
    pub ip: Arc<RateLimiter>,
    /// `/auth/*`, keyed by client IP.
    pub auth: Arc<RateLimiter>,
    /// Authenticated routes, keyed by user id.
    pub user: Arc<RateLimiter>,
}

impl RateLimits {
    pub const IP_PER_MINUTE: u32 = 60;
    pub const AUTH_PER_MINUTE: u32 = 10;
    pub const USER_PER_MINUTE: u32 = 120;

    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self::with_limits(
            clock,
            Self::IP_PER_MINUTE,
            Self::AUTH_PER_MINUTE,
            Self::USER_PER_MINUTE,
        )
    }

    pub fn with_limits(clock: Arc<dyn Clock>, ip: u32, auth: u32, user: u32) -> Self {
        let make = |per_minute| {
            Arc::new(RateLimiter::new(
                RateLimitConfig { per_minute },
                clock.clone(),
            ))
        };
        Self {
            ip: make(ip),
            auth: make(auth),
            user: make(user),
        }
    }
}

impl AppState {
    /// Per-user limit for authenticated routes. The auth extractor calls this
    /// once the session is resolved.
    pub fn check_user_limit(&self, user: &UserId) -> Result<(), AppError> {
        self.limits
            .user
            .check(user.as_str())
            .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })
    }
}

/// Builds the full application router (tests drive it with `oneshot`).
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .merge(routes::extra_routes())
        .fallback(routes::not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::ip_rate_limit,
        ))
        .layer(
            // Log method and path only: never query strings (they carry OAuth
            // codes) and never headers (Authorization).
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "http",
                    method = %request.method(),
                    path = %loggable_target(request.uri()),
                )
            }),
        )
        .with_state(state)
}

/// The part of the URI that may be logged: the path, never the query.
pub fn loggable_target(uri: &Uri) -> String {
    uri.path().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logged_target_drops_the_query_string() {
        let uri: Uri = "/auth/discord/callback?code=secret&state=abc"
            .parse()
            .unwrap();
        assert_eq!(loggable_target(&uri), "/auth/discord/callback");
    }
}
