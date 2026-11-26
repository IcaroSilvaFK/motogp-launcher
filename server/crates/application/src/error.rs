use std::fmt;

/// Error shared by every use case. The API maps it to the JSON error format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    BadRequest(String),
    InvalidRedirect,
    InvalidCode,
    Unauthorized,
    NotFound,
    RateLimited {
        retry_after_secs: u64,
    },
    DiscordUnavailable,
    /// Internal detail: logged, never returned to the client.
    Internal(String),
}

impl AppError {
    /// Stable snake_case code used in the JSON body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::InvalidRedirect => "invalid_redirect",
            Self::InvalidCode => "invalid_code",
            Self::Unauthorized => "unauthorized",
            Self::NotFound => "not_found",
            Self::RateLimited { .. } => "rate_limited",
            Self::DiscordUnavailable => "discord_unavailable",
            Self::Internal(_) => "internal",
        }
    }

    /// English message that is safe to show to clients.
    pub fn public_message(&self) -> String {
        match self {
            Self::BadRequest(message) => message.clone(),
            Self::InvalidRedirect => "The redirect URL is not allowed.".into(),
            Self::InvalidCode => "The code is invalid, expired or already used.".into(),
            Self::Unauthorized => "Authentication is required.".into(),
            Self::NotFound => "The resource was not found.".into(),
            Self::RateLimited { .. } => "Too many requests. Try again later.".into(),
            Self::DiscordUnavailable => "Discord is unavailable. Try again later.".into(),
            Self::Internal(_) => "An internal error occurred.".into(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(detail) => write!(f, "internal: {detail}"),
            other => write!(f, "{}: {}", other.code(), other.public_message()),
        }
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(AppError::InvalidRedirect.code(), "invalid_redirect");
        assert_eq!(
            AppError::RateLimited {
                retry_after_secs: 1
            }
            .code(),
            "rate_limited"
        );
        assert_eq!(AppError::DiscordUnavailable.code(), "discord_unavailable");
    }

    #[test]
    fn internal_detail_is_not_public() {
        let error = AppError::Internal("disk path /secret".into());
        assert!(!error.public_message().contains("secret"));
    }
}
