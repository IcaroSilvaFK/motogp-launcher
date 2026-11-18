//! `ContentHasher` adapter: streaming SHA-256.
use motogp_application::{ContentHasher, LibraryError};
use std::path::Path;

/// Computes the SHA-256 of a file in fixed-size chunks (mods can be large).
#[derive(Debug, Default, Clone, Copy)]
pub struct Sha256Hasher;

impl ContentHasher for Sha256Hasher {
    fn sha256_hex(&self, _file: &Path) -> Result<String, LibraryError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn hashes_known_content_as_lowercase_hex() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("abc.bin");
        fs::write(&file, b"abc").unwrap();
        assert_eq!(Sha256Hasher.sha256_hex(&file).unwrap(), ABC_SHA256);
    }

    #[test]
    fn hashes_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("empty.bin");
        fs::write(&file, b"").unwrap();
        assert_eq!(Sha256Hasher.sha256_hex(&file).unwrap(), EMPTY_SHA256);
    }

    #[test]
    fn streaming_matches_one_shot_for_content_larger_than_the_buffer() {
        use sha2::{Digest, Sha256};
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        let content: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&file, &content).unwrap();
        let expected = format!("{:x}", Sha256::digest(&content));
        assert_eq!(Sha256Hasher.sha256_hex(&file).unwrap(), expected);
    }

    #[test]
    fn missing_file_is_a_read_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = Sha256Hasher
            .sha256_hex(&dir.path().join("missing.bin"))
            .unwrap_err();
        assert!(
            matches!(
                error,
                LibraryError::Storage(motogp_application::AppError::Read { .. })
            ),
            "{error:?}"
        );
    }
}
