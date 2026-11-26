//! Reusable in-memory token-bucket rate limiting.
//!
//! A [`RateLimiter`] is keyed by any string (client IP, user id...). The IP
//! middleware [`ip_rate_limit`] covers every route; authenticated handlers call
//! `AppState::check_user_limit`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, Utc};
use community_application::ports::Clock;
use community_application::AppError;

use crate::{ApiError, AppState};

const SWEEP_INTERVAL_SECS: i64 = 60;

#[derive(Clone, Copy, Debug)]
pub struct RateLimitConfig {
    /// Burst size and refill per minute.
    pub per_minute: u32,
}

struct Bucket {
    tokens: f64,
    updated: DateTime<Utc>,
}

struct Inner {
    buckets: HashMap<String, Bucket>,
    last_sweep: DateTime<Utc>,
}

pub struct RateLimiter {
    config: RateLimitConfig,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig, clock: Arc<dyn Clock>) -> Self {
        let now = clock.now();
        Self {
            config,
            clock,
            inner: Mutex::new(Inner {
                buckets: HashMap::new(),
                last_sweep: now,
            }),
        }
    }

    fn refill_per_sec(&self) -> f64 {
        f64::from(self.config.per_minute) / 60.0
    }

    /// Takes one token for `key`. `Err(retry_after_secs)` when the bucket is empty.
    pub fn check(&self, key: &str) -> Result<(), u64> {
        let now = self.clock.now();
        let capacity = f64::from(self.config.per_minute);
        let rate = self.refill_per_sec();
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());

        if now - inner.last_sweep >= Duration::seconds(SWEEP_INTERVAL_SECS) {
            // A bucket idle long enough to be full again carries no information.
            inner.buckets.retain(|_, bucket| {
                let idle = (now - bucket.updated).num_milliseconds() as f64 / 1000.0;
                bucket.tokens + idle * rate < capacity
            });
            inner.last_sweep = now;
        }

        let bucket = inner.buckets.entry(key.to_owned()).or_insert(Bucket {
            tokens: capacity,
            updated: now,
        });
        let elapsed = (now - bucket.updated).num_milliseconds().max(0) as f64 / 1000.0;
        bucket.tokens = (bucket.tokens + elapsed * rate).min(capacity);
        bucket.updated = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(())
        } else if rate <= 0.0 {
            Err(60)
        } else {
            let wait = ((1.0 - bucket.tokens) / rate).ceil();
            Err((wait as u64).max(1))
        }
    }

    /// Number of tracked buckets (for tests and metrics).
    pub fn tracked_keys(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .buckets
            .len()
    }
}

/// Client IP: the connection address, or the right-most `X-Forwarded-For`
/// value when `trust_proxy_headers` is set (our own reverse proxy appends it).
pub fn client_ip(
    headers: &HeaderMap,
    connection: Option<SocketAddr>,
    trust_proxy_headers: bool,
) -> String {
    if trust_proxy_headers {
        let forwarded = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .rfind(|part| !part.is_empty());
        if let Some(ip) = forwarded {
            return ip.to_owned();
        }
    }
    connection
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Middleware applied to every route: `/auth/*` uses the stricter limiter.
pub async fn ip_rate_limit(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let connection = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0);
    // Big uploads send many chunks; they are limited per user by the auth extractor instead.
    if crate::uploads::is_chunk_request(request.method(), request.uri().path()) {
        return next.run(request).await;
    }
    let ip = client_ip(request.headers(), connection, state.trust_proxy_headers);
    let path = request.uri().path();
    let limiter = if path == "/auth" || path.starts_with("/auth/") {
        &state.limits.auth
    } else {
        &state.limits.ip
    };
    match limiter.check(&ip) {
        Ok(()) => next.run(request).await,
        Err(retry_after_secs) => {
            ApiError(AppError::RateLimited { retry_after_secs }).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeClock;

    fn limiter(per_minute: u32) -> (Arc<FakeClock>, RateLimiter) {
        let clock = Arc::new(FakeClock::at_epoch_plus(1_000));
        let limiter = RateLimiter::new(RateLimitConfig { per_minute }, clock.clone());
        (clock, limiter)
    }

    #[test]
    fn allows_the_burst_then_rejects_with_retry_after() {
        let (_, limiter) = limiter(3);
        for _ in 0..3 {
            assert!(limiter.check("a").is_ok());
        }
        assert_eq!(limiter.check("a"), Err(20));
    }

    #[test]
    fn keys_are_independent() {
        let (_, limiter) = limiter(1);
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_err());
        assert!(limiter.check("b").is_ok());
    }

    #[test]
    fn tokens_refill_over_time() {
        let (clock, limiter) = limiter(60);
        for _ in 0..60 {
            assert!(limiter.check("a").is_ok());
        }
        assert!(limiter.check("a").is_err());
        clock.advance(Duration::seconds(2));
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_ok());
        assert!(limiter.check("a").is_err());
    }

    #[test]
    fn stale_buckets_are_evicted() {
        let (clock, limiter) = limiter(10);
        limiter.check("a").unwrap();
        limiter.check("b").unwrap();
        assert_eq!(limiter.tracked_keys(), 2);
        clock.advance(Duration::seconds(120));
        limiter.check("c").unwrap();
        assert_eq!(limiter.tracked_keys(), 1);
    }

    #[test]
    fn client_ip_uses_the_connection_by_default() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "9.9.9.9".parse().unwrap());
        let addr: SocketAddr = "1.2.3.4:5555".parse().unwrap();
        assert_eq!(client_ip(&headers, Some(addr), false), "1.2.3.4");
        assert_eq!(client_ip(&headers, None, false), "unknown");
    }

    #[test]
    fn client_ip_trusts_the_rightmost_forwarded_value_only_when_enabled() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "6.6.6.6, 7.7.7.7 , 203.0.113.9".parse().unwrap(),
        );
        let addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        assert_eq!(client_ip(&headers, Some(addr), true), "203.0.113.9");
        let empty = HeaderMap::new();
        assert_eq!(client_ip(&empty, Some(addr), true), "127.0.0.1");
    }
}
