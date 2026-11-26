//! Ports of the community account (issue #122).
use crate::{CommunityApiError, CredentialError, LoopbackError};
use motogp_domain::SessionToken;
use std::time::Duration;

/// The account the server knows, as `GET /me` returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerUser {
    pub display_name: String,
    /// Absolute https URL of the avatar, if the user has one.
    pub avatar_url: Option<String>,
}

/// Port: the community server's account endpoints.
pub trait CommunityApi: Send + Sync {
    /// The URL the browser opens to start the Discord sign-in. Pure: no I/O.
    fn start_url(&self, redirect: &str, state: &str, code_challenge: &str) -> String;
    /// Trades the one-time code (plus the PKCE verifier) for a session.
    fn exchange(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<(SessionToken, ServerUser), CommunityApiError>;
    /// The account of the session (`Unauthorized` when it is no longer valid).
    fn me(&self, token: &SessionToken) -> Result<ServerUser, CommunityApiError>;
    /// Revokes the session on the server.
    fn logout(&self, token: &SessionToken) -> Result<(), CommunityApiError>;
}

/// Port: where the session token lives between launches (Windows Credential
/// Manager in production).
pub trait CredentialStore: Send + Sync {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError>;
    fn save(&self, token: &SessionToken) -> Result<(), CredentialError>;
    /// Deleting a token that is not there is a success.
    fn delete(&self) -> Result<(), CredentialError>;
}

/// What the browser brought back to the loopback address.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// A bound loopback listener waiting for one browser callback.
pub trait LoopbackSession: Send {
    /// `http://127.0.0.1:<port>/callback`
    fn redirect_url(&self) -> String;
    /// Blocks until the browser comes back or `timeout` passes.
    fn wait(self: Box<Self>, timeout: Duration) -> Result<CallbackParams, LoopbackError>;
}

/// Port: opens a loopback listener on a free port.
pub trait LoopbackReceiver: Send + Sync {
    fn bind(&self) -> Result<Box<dyn LoopbackSession>, LoopbackError>;
}

/// Port: random values and the PKCE transform (needs a CSPRNG and SHA-256, so
/// it lives outside the application layer).
pub trait PkceSource: Send + Sync {
    /// A fresh url-safe `state` (1 to 128 characters).
    fn state(&self) -> String;
    /// A fresh PKCE verifier.
    fn verifier(&self) -> String;
    /// `BASE64URL(SHA256(verifier))`, 43 characters, no padding.
    fn challenge(&self, verifier: &str) -> String;
}

/// Port: downloads an avatar and returns it as a `data:` URL. Any failure is
/// `None`: the account row falls back to initials.
pub trait AvatarFetcher: Send + Sync {
    fn fetch(&self, url: &str) -> Option<String>;
}
