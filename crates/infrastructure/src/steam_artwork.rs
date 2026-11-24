//! Read-only adapter over Steam's local library cache
//! (`<steam>/appcache/librarycache`). It never writes and never downloads.
use motogp_application::{AppError, Artwork, GameArtworkSource, ImageMime};
use std::fs;
use std::path::{Path, PathBuf};

/// Files above this size are skipped: the artwork travels over IPC as a
/// `data:` URL, so it must stay small.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Artwork slots in order of preference for a small square tile. Each slot
/// lists the file names Steam uses, newest layout names first.
const SLOTS: &[&[&str]] = &[
    // Slot 0 (icon) is matched separately: a 40-hex-digit `.jpg` at the root.
    &[],
    &["library_capsule.jpg", "library_600x900.jpg"],
    &["library_header.jpg", "header.jpg"],
    &["library_hero.jpg"],
    &["logo.png"],
];

/// Local Steam artwork cache.
pub struct SteamArtwork {
    steam_path: Option<PathBuf>,
}

impl SteamArtwork {
    pub fn new(steam_path: Option<PathBuf>) -> Self {
        Self { steam_path }
    }
}

impl GameArtworkSource for SteamArtwork {
    fn find(&self, app_id: u32) -> Result<Option<Artwork>, AppError> {
        let Some(steam) = &self.steam_path else {
            return Ok(None);
        };
        let cache = steam.join("appcache").join("librarycache");
        let folder = cache.join(app_id.to_string());
        let (root_files, sub_files) = folder_files(&folder);
        for (slot, names) in SLOTS.iter().enumerate() {
            let mut candidates: Vec<PathBuf> = Vec::new();
            if slot == 0 {
                candidates.extend(root_files.iter().filter(|p| is_hash_icon(p)).cloned());
                candidates.push(cache.join(format!("{app_id}_icon.jpg")));
            } else {
                for name in *names {
                    candidates.extend(root_files.iter().filter(|p| named(p, name)).cloned());
                    candidates.extend(sub_files.iter().filter(|p| named(p, name)).cloned());
                }
                for name in *names {
                    candidates.push(cache.join(format!("{app_id}_{name}")));
                }
            }
            if let Some(artwork) = candidates.iter().find_map(|p| read_image(p)) {
                return Ok(Some(artwork));
            }
        }
        Ok(None)
    }
}

/// Files directly in the app folder and files one level down (hashed
/// sub-folders), both sorted so the choice is deterministic. IO errors yield
/// fewer candidates: artwork is cosmetic.
fn folder_files(folder: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    if let Ok(entries) = fs::read_dir(folder) {
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    dirs.sort();
    let mut nested = Vec::new();
    for dir in dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut inner: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            inner.sort();
            nested.extend(inner);
        }
    }
    (files, nested)
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

fn named(path: &Path, name: &str) -> bool {
    file_name(path).eq_ignore_ascii_case(name)
}

/// `<40 hex digits>.jpg`: the small community icon.
fn is_hash_icon(path: &Path) -> bool {
    match file_name(path).to_ascii_lowercase().strip_suffix(".jpg") {
        Some(stem) => stem.len() == 40 && stem.bytes().all(|b| b.is_ascii_hexdigit()),
        None => false,
    }
}

/// Reads the file when it is a non-empty image within the size limit.
fn read_image(path: &Path) -> Option<Artwork> {
    let len = fs::metadata(path).ok()?.len();
    if len == 0 || len > MAX_BYTES {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let mime = detect_mime(&bytes)?;
    Some(Artwork { bytes, mime })
}

fn detect_mime(bytes: &[u8]) -> Option<ImageMime> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(ImageMime::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageMime::Jpeg)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageMime::Webp)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "9e5c0000000000000000000000000000000a8b8f";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];

    #[test]
    fn detects_png_jpeg_and_webp_by_content() {
        assert_eq!(
            detect_mime(&[0x89, b'P', b'N', b'G', 0]),
            Some(ImageMime::Png)
        );
        assert_eq!(detect_mime(JPEG), Some(ImageMime::Jpeg));
        assert_eq!(detect_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageMime::Webp));
        assert_eq!(detect_mime(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert_eq!(detect_mime(b"hello"), None);
    }

    #[test]
    fn hash_icon_needs_forty_hex_digits_and_jpg() {
        assert!(is_hash_icon(Path::new(&format!("{HASH}.jpg"))));
        assert!(!is_hash_icon(Path::new("header.jpg")));
        assert!(!is_hash_icon(Path::new(&format!("{HASH}.png"))));
        assert!(!is_hash_icon(Path::new(
            "zzzz0000000000000000000000000000000a8b8f.jpg"
        )));
    }

    #[test]
    fn webp_content_is_served_with_its_own_mime_whatever_the_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("appcache/librarycache/5");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("header.jpg"), b"RIFF\0\0\0\0WEBPVP8 ").unwrap();
        let art = SteamArtwork::new(Some(tmp.path().to_path_buf()))
            .find(5)
            .unwrap()
            .unwrap();
        assert_eq!(art.mime, ImageMime::Webp);
    }

    #[test]
    fn empty_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("appcache/librarycache/5");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{HASH}.jpg")), b"").unwrap();
        fs::write(dir.join("logo.png"), [0x89, b'P', b'N', b'G']).unwrap();
        let art = SteamArtwork::new(Some(tmp.path().to_path_buf()))
            .find(5)
            .unwrap()
            .unwrap();
        assert_eq!(art.mime, ImageMime::Png);
    }

    #[test]
    fn blur_variants_are_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("appcache/librarycache/5/abc");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("library_hero_blur.jpg"), JPEG).unwrap();
        let found = SteamArtwork::new(Some(tmp.path().to_path_buf()))
            .find(5)
            .unwrap();
        assert_eq!(found, None);
    }
}
