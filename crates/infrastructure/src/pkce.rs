//! PKCE and `state` generation for the community sign-in (issue #122), from
//! the operating system's CSPRNG.
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use motogp_application::PkceSource;
use sha2::{Digest, Sha256};

/// [`PkceSource`] backed by `getrandom` and SHA-256.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsPkce;

/// `bytes` random bytes as unpadded base64url. The OS RNG failing is not
/// recoverable (there is no safe fallback for a secret), so it panics; the
/// caller runs inside `spawn_blocking`, which reports it as a failed task.
fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer).expect("the operating system random generator failed");
    URL_SAFE_NO_PAD.encode(buffer)
}

impl PkceSource for OsPkce {
    fn state(&self) -> String {
        random_token(24)
    }

    fn verifier(&self) -> String {
        random_token(32)
    }

    fn challenge(&self, verifier: &str) -> String {
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_matches_the_rfc_7636_example() {
        assert_eq!(
            OsPkce.challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn the_challenge_has_43_url_safe_characters() {
        let challenge = OsPkce.challenge(&OsPkce.verifier());
        assert_eq!(challenge.len(), 43);
        assert!(challenge
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn state_and_verifier_are_url_safe_and_in_range_and_never_repeat() {
        let state = OsPkce.state();
        assert!((1..=128).contains(&state.len()));
        assert!(state
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        let verifier = OsPkce.verifier();
        assert_eq!(verifier.len(), 43);
        assert_ne!(OsPkce.state(), OsPkce.state());
        assert_ne!(verifier, OsPkce.verifier());
    }
}
