//! Local development stand-in for Discord OAuth. It never contacts Discord: the
//! "consent screen" is the server's own callback, which then resolves to a fake
//! user, so the launcher's real sign-in code path runs unchanged.
//!
//! Only wired when `DEV_FAKE_AUTH` is on, and `Config` refuses that unless
//! `PUBLIC_BASE_URL` is a loopback URL.

use community_application::ports::{AccessToken, DiscordOAuth, DiscordUser, PortResult};
use community_application::AppError;

/// The authorization code the fake consent step hands out.
const FAKE_CODE: &str = "dev-fake-code";
const FAKE_ACCESS_TOKEN: &str = "dev-fake-access-token";

#[derive(Debug)]
pub struct DevFakeDiscord {
    /// `{PUBLIC_BASE_URL}/auth/discord/callback`.
    callback_url: String,
    user: DiscordUser,
}

impl DevFakeDiscord {
    pub fn new(public_base_url: &str, username: &str, discord_id: &str) -> Self {
        Self {
            callback_url: format!(
                "{}/auth/discord/callback",
                public_base_url.trim_end_matches('/')
            ),
            user: DiscordUser {
                id: discord_id.to_owned(),
                username: username.to_owned(),
                global_name: Some(username.to_owned()),
                avatar: None,
            },
        }
    }
}

#[async_trait::async_trait]
impl DiscordOAuth for DevFakeDiscord {
    fn authorize_url(&self, state: &str, _code_challenge: &str) -> String {
        format!("{}?code={FAKE_CODE}&state={state}", self.callback_url)
    }

    async fn exchange_code(&self, code: &str, _code_verifier: &str) -> PortResult<AccessToken> {
        if code == FAKE_CODE {
            Ok(AccessToken(FAKE_ACCESS_TOKEN.to_owned()))
        } else {
            Err(AppError::DiscordUnavailable)
        }
    }

    async fn fetch_user(&self, access_token: &AccessToken) -> PortResult<DiscordUser> {
        if access_token.0 == FAKE_ACCESS_TOKEN {
            Ok(self.user.clone())
        } else {
            Err(AppError::DiscordUnavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake() -> DevFakeDiscord {
        DevFakeDiscord::new("http://127.0.0.1:8787/", "dev-user", "900000000000000001")
    }

    #[test]
    fn authorize_url_points_back_at_the_servers_own_callback() {
        let url = fake().authorize_url("st4te", "challenge");
        assert_eq!(
            url,
            "http://127.0.0.1:8787/auth/discord/callback?code=dev-fake-code&state=st4te"
        );
    }

    #[tokio::test]
    async fn resolves_the_configured_user() {
        let discord = fake();
        let token = discord.exchange_code(FAKE_CODE, "verifier").await.unwrap();
        let user = discord.fetch_user(&token).await.unwrap();
        assert_eq!(user.id, "900000000000000001");
        assert_eq!(user.username, "dev-user");
        assert_eq!(user.global_name.as_deref(), Some("dev-user"));
    }

    #[tokio::test]
    async fn refuses_any_other_code_or_token() {
        let discord = fake();
        assert!(matches!(
            discord.exchange_code("real-discord-code", "v").await,
            Err(AppError::DiscordUnavailable)
        ));
        assert!(matches!(
            discord.fetch_user(&AccessToken("other".into())).await,
            Err(AppError::DiscordUnavailable)
        ));
    }
}
