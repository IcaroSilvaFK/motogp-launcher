//! Steps for `tests/features/steam.feature` (issue #5).
//! A temporary Steam tree is used; the real registry is never touched.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{GameError, GameService, SteamLocator};
use motogp_domain::{GameInfo, SteamManifest};
use motogp_infrastructure::{
    normalize_windows_path, parse_app_manifest, parse_library_folders, FsGameFiles, SteamLibraries,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[derive(Debug, Default, World)]
struct SteamWorld {
    tmp: Option<TempDir>,
    no_steam: bool,
    vdf: String,
    expected_paths: Vec<PathBuf>,
    libraries: Vec<PathBuf>,
    path_input: Option<PathBuf>,
    normalized: Option<PathBuf>,
    registry_steam_path: Option<PathBuf>,
    acf: String,
    manifest: Option<SteamManifest>,
    root: Option<PathBuf>,
    inspected: Option<Result<GameInfo, GameError>>,
    detected: Option<Vec<GameInfo>>,
}

impl SteamWorld {
    fn base(&mut self) -> PathBuf {
        self.tmp
            .get_or_insert_with(|| tempfile::tempdir().expect("temp dir"))
            .path()
            .to_path_buf()
    }

    fn steamapps(&mut self) -> PathBuf {
        self.base().join("steamapps")
    }

    fn mkdirs(&mut self, rel: &str) -> PathBuf {
        let p = self.steamapps().join(rel);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn service(&mut self) -> GameService<SteamLibraries, FsGameFiles> {
        let steam = if self.no_steam {
            None
        } else {
            Some(self.base())
        };
        GameService::new(SteamLibraries::new(steam), FsGameFiles)
    }

    fn info(&self) -> &GameInfo {
        self.inspected
            .as_ref()
            .expect("not inspected")
            .as_ref()
            .expect("inspection failed")
    }
}

/// Accepts the text either raw or already unescaped by the step matcher.
fn unescaped(raw: &str) -> String {
    raw.replace("\\\\", "\\")
}

fn vdf_escaped(path: &str) -> String {
    path.replace('\\', "\\\\")
}

// ---------- Parsing ----------

#[given(expr = "a libraryfolders.vdf with {string} and {string} escaped in VDF format")]
fn libraryfolders(w: &mut SteamWorld, first: String, second: String) {
    let (a, b) = (unescaped(&first), unescaped(&second));
    w.vdf = format!(
        "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}",
        vdf_escaped(&a),
        vdf_escaped(&b)
    );
    w.expected_paths = vec![PathBuf::from(a), PathBuf::from(b)];
}

#[when("I read the libraries")]
fn read_libraries(w: &mut SteamWorld) {
    // With a registry path the full locator runs (reads the VDF from disk); otherwise only the parser.
    w.libraries = match w.registry_steam_path.clone() {
        Some(steam) => SteamLibraries::new(Some(steam))
            .library_folders()
            .expect("libraries"),
        None => parse_library_folders(&w.vdf),
    };
}

#[then("I receive both Windows paths with unescaped backslashes")]
fn both_paths(w: &mut SteamWorld) {
    assert_eq!(w.libraries, w.expected_paths);
    assert!(w
        .libraries
        .iter()
        .all(|p| !p.to_string_lossy().contains("\\\\")));
}

#[then("the spaces are preserved")]
fn spaces_preserved(w: &mut SteamWorld) {
    assert!(w.libraries[0]
        .to_string_lossy()
        .contains("Program Files (x86)"));
}

#[given(expr = "an ACF manifest with appid {string} and installdir {string}")]
fn acf_manifest(w: &mut SteamWorld, app_id: String, install_dir: String) {
    w.acf = format!(
        "\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"installdir\"\t\t\"{install_dir}\"\n}}"
    );
}

#[when("I parse the manifest")]
fn parse_manifest(w: &mut SteamWorld) {
    w.manifest = parse_app_manifest(&w.acf);
}

#[then(expr = "I receive the ID {int} and the name {string}")]
fn manifest_result(w: &mut SteamWorld, id: u32, name: String) {
    assert_eq!(
        w.manifest,
        Some(SteamManifest {
            app_id: id,
            install_dir: name
        })
    );
}

// ---------- Temporary Steam tree ----------

#[given(expr = "{string} in a temporary Steam tree")]
fn tree_path(w: &mut SteamWorld, rel: String) {
    w.mkdirs(&rel);
}

#[given(expr = "an appmanifest in steamapps with installdir {string} and appid {string}")]
fn appmanifest(w: &mut SteamWorld, install_dir: String, app_id: String) {
    let dir = w.mkdirs("");
    let acf = format!(
        "\"AppState\"\n{{\n\t\"appid\"\t\t\"{app_id}\"\n\t\"installdir\"\t\t\"{install_dir}\"\n}}"
    );
    fs::write(dir.join(format!("appmanifest_{app_id}.acf")), acf).unwrap();
}

#[given("a root without Content/Paks in its project subfolders")]
fn root_without_paks(w: &mut SteamWorld) {
    let root = w.mkdirs("common/MotoGP25");
    fs::create_dir_all(root.join("MotoGP25").join("Content")).unwrap();
    w.root = Some(root);
}

#[given("libraries with a valid MotoGP25 and a valid OtherGame")]
fn two_games(w: &mut SteamWorld) {
    w.mkdirs("common/MotoGP25/MotoGP25/Content/Paks");
    w.mkdirs("common/OtherGame/Other/Content/Paks");
}

#[given("a root with Engine/Content/Paks and Project/Content/Paks")]
fn engine_and_project(w: &mut SteamWorld) {
    w.mkdirs("common/MotoGP25/Engine/Content/Paks");
    w.mkdirs("common/MotoGP25/Project/Content/Paks");
    w.root = Some(w.steamapps().join("common").join("MotoGP25"));
}

#[given("a valid Unreal root without a matching manifest")]
fn valid_without_manifest(w: &mut SteamWorld) {
    let root = w.mkdirs("common/MotoGP25");
    fs::create_dir_all(root.join("MotoGP25").join("Content").join("Paks")).unwrap();
    w.root = Some(root);
}

#[given("SteamPath is not available")]
fn no_steam(w: &mut SteamWorld) {
    w.no_steam = true;
}

#[when(expr = "I inspect the root {string}")]
fn inspect_relative(w: &mut SteamWorld, rel: String) {
    let root = w.steamapps().join(rel);
    w.inspected = Some(w.service().inspect_root(&root));
    w.root = Some(root);
}

#[when("I inspect it")]
fn inspect_it(w: &mut SteamWorld) {
    let root = w.root.clone().expect("root not set");
    w.inspected = Some(w.service().inspect_root(&root));
}

#[when("I detect the games")]
fn detect(w: &mut SteamWorld) {
    w.detected = Some(w.service().detect_games().expect("detect"));
}

#[then(expr = "mods_dir ends with {string}")]
fn mods_dir_ends(w: &mut SteamWorld, suffix: String) {
    assert!(w.info().mods_dir.ends_with(suffix));
}

#[then(expr = "the App ID is {int}")]
fn app_id_is(w: &mut SteamWorld, id: u32) {
    assert_eq!(w.info().app_id, Some(id));
}

#[then("the ~mods folder is not created")]
fn mods_not_created(w: &mut SteamWorld) {
    assert!(!w.info().mods_dir.exists());
}

#[then(expr = "I receive an error containing {string}")]
fn error_containing(w: &mut SteamWorld, text: String) {
    let err = w
        .inspected
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .expect("expected an error");
    assert!(err.to_string().contains(&text), "got: {err}");
}

#[then("only MotoGP25 appears")]
fn only_motogp(w: &mut SteamWorld) {
    let names: Vec<&str> = w
        .detected
        .as_ref()
        .unwrap()
        .iter()
        .map(|g| g.name.as_str())
        .collect();
    assert_eq!(names, vec!["MotoGP25"]);
}

#[then("the Paks folder used is Project/Content/Paks")]
fn paks_is_project(w: &mut SteamWorld) {
    let root = w.root.clone().unwrap();
    assert_eq!(
        w.info().mods_dir,
        root.join("Project")
            .join("Content")
            .join("Paks")
            .join("~mods")
    );
}

#[then("I receive a GameInfo with no App ID")]
fn no_app_id(w: &mut SteamWorld) {
    assert_eq!(w.info().app_id, None);
}

#[then("no mods folder is created")]
fn no_mods_folder(w: &mut SteamWorld) {
    assert!(!w.info().mods_dir.exists());
}

#[then("I receive an empty list")]
fn empty_list(w: &mut SteamWorld) {
    assert!(w.detected.as_ref().unwrap().is_empty());
}

#[then("manual inspection is still available")]
fn manual_available(w: &mut SteamWorld) {
    let root: &Path = &w.mkdirs("manual/MotoGP25");
    fs::create_dir_all(root.join("P").join("Content").join("Paks")).unwrap();
    let root = root.to_path_buf();
    assert!(w.service().inspect_root(&root).is_ok());
}

// ---------- Path normalization (issue #61) ----------

#[given(expr = "the registry SteamPath is {string}")]
#[given(expr = "the path {string}")]
fn path_input(w: &mut SteamWorld, raw: String) {
    w.path_input = Some(PathBuf::from(raw));
}

#[when("I normalize it")]
fn normalize_it(w: &mut SteamWorld) {
    let input = w.path_input.clone().expect("path not set");
    w.normalized = Some(normalize_windows_path(&input));
}

#[then(expr = "I receive {string}")]
fn receive_normalized(w: &mut SteamWorld, expected: String) {
    assert_eq!(w.normalized, Some(PathBuf::from(expected)));
}

/// Real mixed-case Steam folder inside the temp dir; the registry spelling differs from it.
fn mixed_case_steam_root(w: &mut SteamWorld) -> PathBuf {
    let root = w.base().join("SteamRoot");
    fs::create_dir_all(root.join("steamapps")).unwrap();
    root
}

#[given("the registry SteamPath is a lower-cased forward-slash spelling of the Steam folder")]
fn registry_lowercase_forward_slashes(w: &mut SteamWorld) {
    let root = mixed_case_steam_root(w);
    w.registry_steam_path = Some(PathBuf::from(
        root.to_string_lossy().replace('\\', "/").to_lowercase(),
    ));
}

#[given(expr = "a libraryfolders.vdf that lists the real spelling plus {string}")]
fn vdf_lists_real_spelling(w: &mut SteamWorld, second: String) {
    let real = mixed_case_steam_root(w);
    let second = unescaped(&second);
    w.vdf = format!(
        "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}",
        vdf_escaped(&real.to_string_lossy()),
        vdf_escaped(&second)
    );
    fs::write(real.join("steamapps").join("libraryfolders.vdf"), &w.vdf).unwrap();
}

#[then(expr = "the first library has the VDF spelling and the second is {string}")]
fn libraries_use_vdf_spelling(w: &mut SteamWorld, second: String) {
    // Finding the VDF through a lower-cased path needs a case-insensitive filesystem.
    #[cfg(windows)]
    {
        let real = normalize_windows_path(&mixed_case_steam_root(w));
        // Compared as strings: a case-insensitive path comparison would hide the bug.
        assert_eq!(
            w.libraries[0].to_string_lossy(),
            real.to_string_lossy(),
            "first library must keep the VDF spelling"
        );
        assert_eq!(w.libraries[1], PathBuf::from(second));
    }
    #[cfg(not(windows))]
    let _ = (w, second);
}

#[then("no path contains a forward slash")]
fn no_forward_slash(w: &mut SteamWorld) {
    // Unix temp trees legitimately use `/`; the check only applies to Windows paths.
    if cfg!(windows) {
        assert!(w
            .libraries
            .iter()
            .all(|p| !p.to_string_lossy().contains('/')));
    }
}

/// Runs only `steam.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = SteamWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("steam.feature"))
        .await;
    !writer.execution_has_failed()
}
