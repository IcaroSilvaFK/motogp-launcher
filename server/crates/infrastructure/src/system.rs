//! Adapters for the clock, randomness and hashing ports.

use std::fmt::Write;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use community_application::ports::{Clock, Hasher, TokenGenerator};
use sha2::{Digest, Sha256};

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Random strings from the operating system RNG.
pub struct OsTokenGenerator;

impl TokenGenerator for OsTokenGenerator {
    fn generate(&self, byte_len: usize) -> String {
        let mut bytes = vec![0u8; byte_len];
        getrandom::fill(&mut bytes).expect("the OS random number generator must be available");
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

/// Lowercase hex SHA-256.
pub struct Sha256Hasher;

impl Hasher for Sha256Hasher {
    fn hash(&self, input: &str) -> String {
        let digest = Sha256::digest(input.as_bytes());
        let mut out = String::with_capacity(64);
        for byte in digest {
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            Sha256Hasher.hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generator_returns_base64url_of_requested_entropy() {
        let token = OsTokenGenerator.generate(32);
        assert_eq!(token.len(), 43);
        assert!(token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(token, OsTokenGenerator.generate(32));
    }

    #[test]
    fn system_clock_is_close_to_now() {
        let diff = (SystemClock.now() - Utc::now()).num_seconds().abs();
        assert!(diff < 2);
    }
}
