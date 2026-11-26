//! Unified JSON error: `{"error":{"code","message"}}`.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use community_application::AppError;
use serde_json::json;

/// Wrapper that turns an [`AppError`] into an HTTP response.
#[derive(Debug)]
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

pub fn status_of(error: &AppError) -> StatusCode {
    match error {
        AppError::BadRequest(_) | AppError::InvalidRedirect | AppError::InvalidCode => {
            StatusCode::BAD_REQUEST
        }
        AppError::Unauthorized => StatusCode::UNAUTHORIZED,
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
        AppError::DiscordUnavailable => StatusCode::BAD_GATEWAY,
        AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error = self.0;
        if let AppError::Internal(detail) = &error {
            // Detail stays in the log; the client only sees a generic message.
            tracing::error!(detail = %detail, "internal error");
        }
        let body = json!({
            "error": { "code": error.code(), "message": error.public_message() }
        });
        let mut response = (status_of(&error), Json(body)).into_response();
        if let AppError::RateLimited { retry_after_secs } = error {
            if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn render(error: AppError) -> (StatusCode, Response, serde_json::Value) {
        let response = ApiError(error).into_response();
        let status = response.status();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap();
        (
            status,
            Response::from_parts(parts, axum::body::Body::empty()),
            json,
        )
    }

    #[tokio::test]
    async fn maps_every_error_to_status_and_code() {
        let cases = [
            (AppError::BadRequest("x".into()), 400, "bad_request"),
            (AppError::InvalidRedirect, 400, "invalid_redirect"),
            (AppError::InvalidCode, 400, "invalid_code"),
            (AppError::Unauthorized, 401, "unauthorized"),
            (AppError::NotFound, 404, "not_found"),
            (
                AppError::RateLimited {
                    retry_after_secs: 7,
                },
                429,
                "rate_limited",
            ),
            (AppError::DiscordUnavailable, 502, "discord_unavailable"),
            (AppError::Internal("boom".into()), 500, "internal"),
        ];
        for (error, status, code) in cases {
            let (actual, _, json) = render(error).await;
            assert_eq!(actual.as_u16(), status);
            assert_eq!(json["error"]["code"], code);
            assert!(json["error"]["message"].is_string());
        }
    }

    #[tokio::test]
    async fn rate_limited_sets_retry_after_and_internal_hides_detail() {
        let (_, response, _) = render(AppError::RateLimited {
            retry_after_secs: 7,
        })
        .await;
        assert_eq!(response.headers()[header::RETRY_AFTER], "7");
        let (_, _, json) = render(AppError::Internal("secret path".into())).await;
        assert!(!json.to_string().contains("secret path"));
    }
}
