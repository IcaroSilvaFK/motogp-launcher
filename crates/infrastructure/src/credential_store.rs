//! Session token storage in the Windows Credential Manager (issue #122).
//!
//! Dependency: `keyring` (pinned, `windows-native` feature only). It wraps the
//! Win32 `CredWriteW`, `CredReadW` and `CredDeleteW` calls, which would
//! otherwise need hand-written `unsafe` FFI here. The secret never goes into a
//! file, an environment variable or a log.
use keyring::Entry;
use motogp_application::{CredentialError, CredentialStore};
use motogp_domain::SessionToken;

/// Credential Manager target name of the launcher's community session.
pub const COMMUNITY_TARGET: &str = "motogp-launcher-community";
const SERVICE: &str = "motogp-launcher";
const USER: &str = "community-session";

/// What the credential store needs from the OS, so the mapping logic is
/// testable without touching the real Credential Manager.
pub(crate) trait SecretBackend: Send + Sync {
    /// `Ok(None)` when there is no secret.
    fn get(&self) -> Result<Option<String>, String>;
    fn set(&self, secret: &str) -> Result<(), String>;
    /// Deleting a missing secret is a success.
    fn delete(&self) -> Result<(), String>;
}

struct KeyringBackend {
    entry: Entry,
}

impl KeyringBackend {
    fn new() -> Result<Self, String> {
        Entry::new_with_target(COMMUNITY_TARGET, SERVICE, USER)
            .map(|entry| Self { entry })
            .map_err(|e| e.to_string())
    }
}

impl SecretBackend for KeyringBackend {
    fn get(&self) -> Result<Option<String>, String> {
        match self.entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn set(&self, secret: &str) -> Result<(), String> {
        self.entry.set_password(secret).map_err(|e| e.to_string())
    }

    fn delete(&self) -> Result<(), String> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// [`CredentialStore`] on the Windows Credential Manager.
pub struct WindowsCredentialStore {
    backend: Box<dyn SecretBackend>,
}

impl WindowsCredentialStore {
    pub fn new() -> Result<Self, CredentialError> {
        let backend = KeyringBackend::new().map_err(CredentialError)?;
        Ok(Self {
            backend: Box::new(backend),
        })
    }

    #[cfg(test)]
    fn with_backend(backend: Box<dyn SecretBackend>) -> Self {
        Self { backend }
    }
}

impl CredentialStore for WindowsCredentialStore {
    fn load(&self) -> Result<Option<SessionToken>, CredentialError> {
        let secret = self.backend.get().map_err(CredentialError)?;
        // An empty secret is as good as none.
        Ok(secret
            .filter(|value| !value.is_empty())
            .map(SessionToken::new))
    }

    fn save(&self, token: &SessionToken) -> Result<(), CredentialError> {
        self.backend.set(token.expose()).map_err(CredentialError)
    }

    fn delete(&self) -> Result<(), CredentialError> {
        self.backend.delete().map_err(CredentialError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryBackend {
        secret: Mutex<Option<String>>,
        fail: bool,
    }

    impl SecretBackend for MemoryBackend {
        fn get(&self) -> Result<Option<String>, String> {
            if self.fail {
                return Err("locked".into());
            }
            Ok(self.secret.lock().unwrap().clone())
        }
        fn set(&self, secret: &str) -> Result<(), String> {
            if self.fail {
                return Err("locked".into());
            }
            *self.secret.lock().unwrap() = Some(secret.to_string());
            Ok(())
        }
        fn delete(&self) -> Result<(), String> {
            if self.fail {
                return Err("locked".into());
            }
            *self.secret.lock().unwrap() = None;
            Ok(())
        }
    }

    fn store(backend: MemoryBackend) -> WindowsCredentialStore {
        WindowsCredentialStore::with_backend(Box::new(backend))
    }

    #[test]
    fn a_saved_token_loads_back_and_deleting_forgets_it() {
        let store = store(MemoryBackend::default());
        assert_eq!(store.load(), Ok(None));
        store.save(&SessionToken::new("tok")).unwrap();
        assert_eq!(store.load().unwrap().unwrap().expose(), "tok");
        store.delete().unwrap();
        assert_eq!(store.load(), Ok(None));
        // Deleting again is fine.
        assert_eq!(store.delete(), Ok(()));
    }

    #[test]
    fn an_empty_secret_is_no_token() {
        let backend = MemoryBackend::default();
        *backend.secret.lock().unwrap() = Some(String::new());
        assert_eq!(store(backend).load(), Ok(None));
    }

    #[test]
    fn backend_failures_become_credential_errors_without_the_token() {
        let store = store(MemoryBackend {
            fail: true,
            ..Default::default()
        });
        assert_eq!(store.load(), Err(CredentialError("locked".into())));
        let error = store.save(&SessionToken::new("tok-SECRET")).unwrap_err();
        assert!(!error.to_string().contains("tok-SECRET"));
        assert_eq!(store.delete(), Err(CredentialError("locked".into())));
    }

    /// Real round trip on the Windows Credential Manager. Ignored by default
    /// because it touches the user's credential vault.
    #[test]
    #[ignore = "uses the real Windows Credential Manager"]
    fn the_real_credential_manager_round_trips() {
        let backend = KeyringBackend {
            entry: Entry::new_with_target(
                "motogp-launcher-community-test",
                "motogp-launcher",
                "round-trip-test",
            )
            .unwrap(),
        };
        let store = WindowsCredentialStore::with_backend(Box::new(backend));
        store.save(&SessionToken::new("round-trip")).unwrap();
        assert_eq!(store.load().unwrap().unwrap().expose(), "round-trip");
        store.delete().unwrap();
        assert_eq!(store.load(), Ok(None));
    }
}
