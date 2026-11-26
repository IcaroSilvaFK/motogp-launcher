//! Community server domain: plain types and rules, no IO.

use std::fmt;

use chrono::{DateTime, Duration, Utc};

/// How long a login session stays valid.
pub const SESSION_LIFETIME_DAYS: i64 = 30;

/// Discord snowflake of a user (digits only).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiscordId(String);

impl DiscordId {
    /// Returns `None` unless the value is 1 to 25 ASCII digits.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let valid =
            !value.is_empty() && value.len() <= 25 && value.bytes().all(|b| b.is_ascii_digit());
        valid.then(|| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Internal user id (opaque string, never the Discord id).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

impl UserId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A bearer session token. Its `Debug` never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The raw secret. Only hand it to the client or to the hasher.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

/// A community user, created on first Discord sign-in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub discord_id: DiscordId,
    pub username: String,
    pub global_name: Option<String>,
    pub avatar_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Name shown to others: the Discord global name, else the username.
    pub fn display_name(&self) -> &str {
        self.global_name
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.username)
    }
}

/// A stored session. Only the hash of the token is kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub user_id: UserId,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl Session {
    pub fn expiry_from(created_at: DateTime<Utc>) -> DateTime<Utc> {
        created_at + Duration::days(SESSION_LIFETIME_DAYS)
    }

    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && now < self.expires_at
    }
}

/// An in-progress Discord OAuth sign-in (consumed by the callback).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthFlow {
    /// The `state` sent to Discord (lookup key).
    pub state: String,
    pub discord_verifier: String,
    pub redirect: String,
    pub app_state: String,
    pub app_challenge: String,
    pub expires_at: DateTime<Utc>,
}

/// A short-lived, single-use code handed to the app through the loopback redirect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneTimeCode {
    pub code_hash: String,
    pub user_id: UserId,
    pub app_challenge: String,
    pub expires_at: DateTime<Utc>,
}

/// A validated loopback redirect: exactly `http://127.0.0.1:<1024-65535>/callback`
/// (no userinfo, other host, other path, query or fragment).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoopbackRedirect {
    port: u16,
}

impl LoopbackRedirect {
    const PREFIX: &'static str = "http://127.0.0.1:";
    const PATH: &'static str = "/callback";

    pub fn parse(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(Self::PREFIX)?;
        let digits = rest.strip_suffix(Self::PATH)?;
        let valid_digits = !digits.is_empty()
            && digits.len() <= 5
            && !digits.starts_with('0')
            && digits.bytes().all(|b| b.is_ascii_digit());
        if !valid_digits {
            return None;
        }
        let port: u16 = digits.parse().ok()?;
        (port >= 1024).then_some(Self { port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn as_url(&self) -> String {
        format!("{}{}{}", Self::PREFIX, self.port, Self::PATH)
    }
}

/// PKCE (RFC 7636) with the S256 method.
pub mod pkce {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use sha2::{Digest, Sha256};

    /// `BASE64URL(SHA256(verifier))`.
    pub fn challenge_for(verifier: &str) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
    }

    /// True when the verifier matches the challenge (constant-time comparison).
    pub fn verify(verifier: &str, challenge: &str) -> bool {
        let expected = challenge_for(verifier);
        let (a, b) = (expected.as_bytes(), challenge.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
    }
}

/// A client-chosen opaque value (state, PKCE challenge) that is safe to put in
/// a URL without encoding: 1..=`max_len` characters of `A-Z a-z 0-9 - . _ ~`.
pub fn is_url_safe_token(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_redirect_accepts_only_the_exact_form() {
        let ok = LoopbackRedirect::parse("http://127.0.0.1:54321/callback").unwrap();
        assert_eq!(ok.port(), 54321);
        assert_eq!(ok.as_url(), "http://127.0.0.1:54321/callback");
        assert!(LoopbackRedirect::parse("http://127.0.0.1:1024/callback").is_some());
        assert!(LoopbackRedirect::parse("http://127.0.0.1:65535/callback").is_some());
    }

    #[test]
    fn loopback_redirect_rejects_everything_else() {
        let rejected = [
            "",
            "https://127.0.0.1:5000/callback",
            "https://evil.example/callback",
            "http://localhost:5000/callback",
            "http://127.0.0.2:5000/callback",
            "http://[::1]:5000/callback",
            "http://127.0.0.1/callback",
            "http://127.0.0.1:/callback",
            "http://127.0.0.1:1023/callback",
            "http://127.0.0.1:0/callback",
            "http://127.0.0.1:65536/callback",
            "http://127.0.0.1:99999/callback",
            "http://127.0.0.1:123456/callback",
            "http://127.0.0.1:05000/callback",
            "http://127.0.0.1:+5000/callback",
            "http://127.0.0.1:50a0/callback",
            "http://127.0.0.1:5000/other",
            "http://127.0.0.1:5000/callback/",
            "http://127.0.0.1:5000/callback?x=1",
            "http://127.0.0.1:5000/callback#frag",
            "http://127.0.0.1:5000",
            "http://user@127.0.0.1:5000/callback",
            "http://user:pw@127.0.0.1:5000/callback",
            "http://127.0.0.1:5000@evil.example/callback",
            "http://evil.example:5000@127.0.0.1:5000/callback",
            "http://127.0.0.1:5000/callback\n",
            " http://127.0.0.1:5000/callback",
            "HTTP://127.0.0.1:5000/callback",
            "//127.0.0.1:5000/callback",
        ];
        for value in rejected {
            assert!(
                LoopbackRedirect::parse(value).is_none(),
                "should reject {value:?}"
            );
        }
    }

    #[test]
    fn pkce_matches_the_rfc_7636_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(pkce::challenge_for(verifier), challenge);
        assert!(pkce::verify(verifier, challenge));
        assert!(!pkce::verify("wrong", challenge));
        assert!(!pkce::verify(verifier, "short"));
    }

    #[test]
    fn url_safe_tokens() {
        assert!(is_url_safe_token("abc-DEF_1.2~", 64));
        assert!(!is_url_safe_token("", 64));
        assert!(!is_url_safe_token("a b", 64));
        assert!(!is_url_safe_token("a&b=c", 64));
        assert!(!is_url_safe_token("%41", 64));
        assert!(!is_url_safe_token(&"a".repeat(65), 64));
    }

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn discord_id_accepts_digits_only() {
        assert!(DiscordId::parse("123456789012345678").is_some());
        assert!(DiscordId::parse(" 42 ").is_some());
        assert!(DiscordId::parse("").is_none());
        assert!(DiscordId::parse("12a").is_none());
        assert!(DiscordId::parse(&"1".repeat(26)).is_none());
    }

    #[test]
    fn session_token_debug_is_redacted() {
        let token = SessionToken::new("super-secret");
        assert!(!format!("{token:?}").contains("super-secret"));
        assert_eq!(token.expose(), "super-secret");
    }

    #[test]
    fn display_name_prefers_global_name() {
        let mut user = User {
            id: UserId("u1".into()),
            discord_id: DiscordId::parse("1").unwrap(),
            username: "rider".into(),
            global_name: Some("Rider One".into()),
            avatar_hash: None,
            created_at: at(0),
        };
        assert_eq!(user.display_name(), "Rider One");
        user.global_name = Some(String::new());
        assert_eq!(user.display_name(), "rider");
        user.global_name = None;
        assert_eq!(user.display_name(), "rider");
    }

    #[test]
    fn session_expires_after_30_days_and_when_revoked() {
        let created = at(1_000);
        let mut session = Session {
            id: "s".into(),
            user_id: UserId("u".into()),
            token_hash: "h".into(),
            created_at: created,
            expires_at: Session::expiry_from(created),
            revoked_at: None,
        };
        assert!(session.is_active(created));
        assert!(!session.is_active(created + Duration::days(30)));
        session.revoked_at = Some(created);
        assert!(!session.is_active(created));
    }
}
