//! Steps for `tests/features/steam-artwork.feature` (issue #70).
//! A temporary Steam tree is used; the real Steam folder is never touched.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{Artwork, GameArtworkSource, ImageMime};
use motogp_infrastructure::SteamArtwork;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

const APP: &str = "3875050";
const HASH: &str = "9e5c0000000000000000000000000000000a8b8f";
const SUB: &str = "1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4d5e6f1a2b";

const ICON: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, b'i', b'c', b'o', b'n'];
const CAPSULE: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, b'c', b'a', b'p'];
const HEADER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, b'h', b'd', b'r'];
const LOGO: &[u8] = &[0x89, b'P', b'N', b'G', b'l', b'o', b'g', b'o'];

#[derive(Debug, Default, World)]
struct ArtworkWorld {
    tmp: Option<TempDir>,
    /// Folder under the temp dir that holds the Steam root.
    steam_dir: Option<String>,
    no_steam: bool,
    found: Option<Option<Artwork>>,
}

impl ArtworkWorld {
    fn steam(&mut self) -> PathBuf {
        let tmp = self
            .tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"));
        tmp.path()
            .join(self.steam_dir.as_deref().unwrap_or("Steam"))
    }

    fn app_dir(&mut self) -> PathBuf {
        let dir = self.steam().join("appcache").join("librarycache").join(APP);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sub_dir(&mut self) -> PathBuf {
        let dir = self.app_dir().join(SUB);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn result(&self) -> &Option<Artwork> {
        self.found.as_ref().expect("no lookup was made")
    }
}

#[given("a Steam cache with an icon and a capsule for App ID 3875050")]
fn icon_and_capsule(w: &mut ArtworkWorld) {
    fs::write(w.app_dir().join(format!("{HASH}.jpg")), ICON).unwrap();
    fs::write(w.sub_dir().join("library_capsule.jpg"), CAPSULE).unwrap();
}

#[given("a Steam cache with only a capsule and a logo in a hashed sub-folder for App ID 3875050")]
fn capsule_and_logo(w: &mut ArtworkWorld) {
    fs::write(w.sub_dir().join("library_capsule.jpg"), CAPSULE).unwrap();
    fs::write(w.sub_dir().join("logo.png"), LOGO).unwrap();
}

#[given(expr = "a Steam cache with only the flat file {string}")]
fn flat_file(w: &mut ArtworkWorld, name: String) {
    let cache = w.steam().join("appcache").join("librarycache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join(name), CAPSULE).unwrap();
}

// "Usuário Padrão" is deliberately accented: it tests Windows user folders
// with non-ASCII characters, which real users have.
#[given(expr = "the Steam root is inside {string}")]
fn steam_inside(w: &mut ArtworkWorld, folder: String) {
    w.steam_dir = Some(format!("{folder}/Steam"));
}

#[given("the cache has an icon for App ID 3875050")]
fn cache_has_icon(w: &mut ArtworkWorld) {
    fs::write(w.app_dir().join(format!("{HASH}.jpg")), ICON).unwrap();
}

#[given("the icon is larger than 2 MiB and the capsule is a text file renamed to .jpg")]
fn big_icon_fake_capsule(w: &mut ArtworkWorld) {
    let big = vec![0xFFu8; 2 * 1024 * 1024 + 1];
    fs::write(w.app_dir().join(format!("{HASH}.jpg")), big).unwrap();
    fs::write(w.sub_dir().join("library_capsule.jpg"), b"just some text").unwrap();
}

#[given("a valid header.jpg exists")]
fn valid_header(w: &mut ArtworkWorld) {
    fs::write(w.sub_dir().join("header.jpg"), HEADER).unwrap();
}

#[given("a Steam cache without files for App ID 3875050")]
fn empty_cache(w: &mut ArtworkWorld) {
    let other = w.steam().join("appcache").join("librarycache").join("999");
    fs::create_dir_all(other).unwrap();
}

#[given("SteamPath is not available")]
fn no_steam(w: &mut ArtworkWorld) {
    w.no_steam = true;
}

#[when("I look up the artwork for App ID 3875050")]
fn look_up(w: &mut ArtworkWorld) {
    let steam = if w.no_steam { None } else { Some(w.steam()) };
    w.found = Some(SteamArtwork::new(steam).find(3875050).expect("lookup"));
}

#[then(expr = "I receive the icon bytes as {string}")]
fn icon_with_mime(w: &mut ArtworkWorld, mime: String) {
    let art = w.result().clone().expect("artwork expected");
    assert_eq!(art.bytes, ICON);
    assert_eq!(art.mime.as_str(), mime);
}

#[then("I receive the capsule image")]
fn capsule(w: &mut ArtworkWorld) {
    assert_eq!(w.result().clone().expect("artwork").bytes, CAPSULE);
}

#[then("I receive that image")]
fn that_image(w: &mut ArtworkWorld) {
    let art = w.result().clone().expect("artwork");
    assert_eq!(art.bytes, CAPSULE);
    assert_eq!(art.mime, ImageMime::Jpeg);
}

#[then("I receive the header image")]
fn header(w: &mut ArtworkWorld) {
    assert_eq!(w.result().clone().expect("artwork").bytes, HEADER);
}

#[then("I receive no artwork")]
fn none(w: &mut ArtworkWorld) {
    assert_eq!(w.result(), &None);
}

/// Runs only `steam-artwork.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ArtworkWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("steam-artwork.feature"))
        .await;
    !writer.execution_has_failed()
}
