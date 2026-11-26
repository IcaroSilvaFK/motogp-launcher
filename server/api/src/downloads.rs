//! Download counts: `POST /mods/{id}/downloads`, `GET /mods/downloads` and
//! `GET /mods/{id}/downloads/daily`. All anonymous.
//!
//! Reports are limited per client IP by a dedicated limiter on top of the
//! global one. The install id is never logged.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::{Extensions, HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use community_application::ports::Clock;
use community_application::{AppError, DownloadService};
use community_domain::downloads::date_of;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::rate_limit::client_ip;
use crate::{ApiError, AppState, RateLimitConfig, RateLimiter};

/// Everything the download routes need, held in one `AppState` field.
#[derive(Clone)]
pub struct DownloadsState {
    pub service: Arc<DownloadService>,
    /// Per client IP, only for `POST /mods/{id}/downloads`.
    pub report_limiter: Arc<RateLimiter>,
}

impl DownloadsState {
    pub const REPORTS_PER_MINUTE: u32 = 20;

    pub fn new(service: Arc<DownloadService>, clock: Arc<dyn Clock>) -> Self {
        Self::with_limit(service, clock, Self::REPORTS_PER_MINUTE)
    }

    pub fn with_limit(
        service: Arc<DownloadService>,
        clock: Arc<dyn Clock>,
        per_minute: u32,
    ) -> Self {
        Self {
            service,
            report_limiter: Arc::new(RateLimiter::new(RateLimitConfig { per_minute }, clock)),
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mods/downloads", get(totals))
        .route("/mods/{id}/downloads", post(report))
        .route("/mods/{id}/downloads/daily", get(daily))
}

#[derive(Deserialize)]
struct ReportBody {
    install_id: String,
}

/// `POST /mods/{id}/downloads`: 204 whether counted now or already counted today.
async fn report(
    State(app): State<AppState>,
    extensions: Extensions,
    headers: HeaderMap,
    Path(mod_id): Path<String>,
    body: Result<Json<ReportBody>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let connection = extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0);
    let ip = client_ip(&headers, connection, app.trust_proxy_headers);
    app.downloads
        .report_limiter
        .check(&ip)
        .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })?;
    let Json(body) = body.map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))?;
    app.downloads
        .service
        .report(&mod_id, &body.install_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /mods/downloads`: totals of every mod with at least one download.
async fn totals(State(app): State<AppState>) -> Result<Json<Value>, ApiError> {
    let mods: Vec<Value> = app
        .downloads
        .service
        .totals()
        .await?
        .into_iter()
        .map(|m| json!({ "id": m.id, "total": m.total, "last_30_days": m.last_30_days }))
        .collect();
    Ok(Json(json!({ "mods": mods })))
}

/// `GET /mods/{id}/downloads/daily`: 30 UTC days, oldest first, ending today.
async fn daily(
    State(app): State<AppState>,
    Path(mod_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let days: Vec<Value> = app
        .downloads
        .service
        .daily(&mod_id)
        .await?
        .into_iter()
        .map(|b| json!({ "date": date_of(b.day), "count": b.count }))
        .collect();
    Ok(Json(json!({ "id": mod_id, "days": days })))
}
