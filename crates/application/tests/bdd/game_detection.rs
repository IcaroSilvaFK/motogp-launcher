//! Steps for `tests/features/game-detection.feature` (issue #5), fake ports only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{AppError, GameError, GameFiles, GameService, SteamLocator};
use motogp_domain::{GameInfo, SteamManifest};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Default)]
struct Tree {
    dirs: BTreeSet<PathBuf>,
    manifests: Vec<SteamManifest>,
    libraries: Vec<PathBuf>,
}

impl Tree {
    fn add(&mut self, path: &str) {
        let mut p = PathBuf::from(path);
        loop {
            self.dirs.insert(p.clone());
            if !p.pop() || p.as_os_str().is_empty() {
                break;
            }
        }
    }
    fn children(&self, dir: &Path) -> Vec<PathBuf> {
        self.dirs
            .iter()
            .filter(|d| d.parent() == Some(dir))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Tree>>);

impl SteamLocator for Fake {
    fn library_folders(&self) -> Result<Vec<PathBuf>, AppError> {
        Ok(self.0.borrow().libraries.clone())
    }
}

impl GameFiles for Fake {
    fn common_dirs(&self, library: &Path) -> Result<Vec<PathBuf>, AppError> {
        Ok(self
            .0
            .borrow()
            .children(&library.join("steamapps").join("common")))
    }
    fn subdirs(&self, dir: &Path) -> Result<Vec<String>, AppError> {
        Ok(self
            .0
            .borrow()
            .children(dir)
            .iter()
            .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
            .collect())
    }
    fn has_paks(&self, project_dir: &Path) -> bool {
        self.0
            .borrow()
            .dirs
            .contains(&project_dir.join("Content").join("Paks"))
    }
    fn manifests(&self, _steamapps: &Path) -> Result<Vec<SteamManifest>, AppError> {
        Ok(self.0.borrow().manifests.clone())
    }
}

#[derive(Default, World)]
struct GameWorld {
    fake: Fake,
    inspected: Option<Result<GameInfo, GameError>>,
    detected: Option<Vec<GameInfo>>,
}

impl std::fmt::Debug for GameWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GameWorld")
    }
}

impl GameWorld {
    fn service(&self) -> GameService<Fake, Fake> {
        GameService::new(self.fake.clone(), self.fake.clone())
    }
    fn info(&self) -> &GameInfo {
        self.inspected
            .as_ref()
            .expect("not inspected")
            .as_ref()
            .expect("inspection failed")
    }
}

#[given(expr = "the game folder {string} has the project {string} with Content\\/Paks")]
fn game_with_project(w: &mut GameWorld, root: String, project: String) {
    w.fake
        .0
        .borrow_mut()
        .add(&format!("{root}/{project}/Content/Paks"));
}

#[given(expr = "a Steam manifest with installdir {string} and appid {string}")]
fn steam_manifest(w: &mut GameWorld, install_dir: String, app_id: String) {
    w.fake.0.borrow_mut().manifests.push(SteamManifest {
        app_id: app_id.parse().unwrap(),
        install_dir,
    });
}

#[given(expr = "a root {string} without Content\\/Paks in its project folders")]
fn root_without_paks(w: &mut GameWorld, root: String) {
    w.fake
        .0
        .borrow_mut()
        .add(&format!("{root}/Project/Content"));
}

#[given(expr = "libraries with a valid {string} and a valid {string}")]
fn two_valid_games(w: &mut GameWorld, first: String, second: String) {
    let mut tree = w.fake.0.borrow_mut();
    tree.libraries.push(PathBuf::from("lib"));
    for name in [first, second] {
        tree.add(&format!("lib/steamapps/common/{name}/{name}/Content/Paks"));
    }
}

#[given(expr = "a root {string} with Engine\\/Content\\/Paks and Bike\\/Content\\/Paks")]
fn engine_and_bike(w: &mut GameWorld, root: String) {
    let mut tree = w.fake.0.borrow_mut();
    tree.add(&format!("{root}/Engine/Content/Paks"));
    tree.add(&format!("{root}/Bike/Content/Paks"));
}

#[given("SteamPath is not available")]
fn no_steam(w: &mut GameWorld) {
    w.fake.0.borrow_mut().libraries.clear();
}

#[when(expr = "I inspect the root {string}")]
fn inspect(w: &mut GameWorld, root: String) {
    w.inspected = Some(w.service().inspect_root(Path::new(&root)));
}

#[when("I detect the games")]
fn detect(w: &mut GameWorld) {
    w.detected = Some(w.service().detect_games().expect("detect"));
}

#[then(expr = "mods_dir ends with {string}")]
fn mods_dir_ends(w: &mut GameWorld, suffix: String) {
    assert!(w.info().mods_dir.ends_with(suffix));
}

#[then(expr = "the App ID is {int}")]
fn app_id_is(w: &mut GameWorld, id: u32) {
    assert_eq!(w.info().app_id, Some(id));
}

#[then(expr = "I receive an error containing {string}")]
fn error_containing(w: &mut GameWorld, text: String) {
    let err = w
        .inspected
        .as_ref()
        .and_then(|r| r.as_ref().err())
        .expect("expected an error");
    assert!(err.to_string().contains(&text), "got: {err}");
}

#[then(expr = "only {string} appears")]
fn only_game(w: &mut GameWorld, name: String) {
    let names: Vec<_> = w
        .detected
        .as_ref()
        .unwrap()
        .iter()
        .map(|g| &g.name)
        .collect();
    assert_eq!(names, vec![&name]);
}

#[then(expr = "the Paks folder used belongs to {string}")]
fn paks_belongs_to(w: &mut GameWorld, project: String) {
    let mods = w.info().mods_dir.to_string_lossy().into_owned();
    assert!(mods.contains(&project), "{mods}");
    assert!(!mods.contains("Engine"), "{mods}");
}

#[then("I receive a GameInfo with no App ID")]
fn no_app_id(w: &mut GameWorld) {
    assert_eq!(w.info().app_id, None);
}

#[then("I receive an empty list")]
fn empty_list(w: &mut GameWorld) {
    assert!(w.detected.as_ref().unwrap().is_empty());
}

#[then("manual inspection is still available")]
fn manual_available(w: &mut GameWorld) {
    w.fake.0.borrow_mut().add("manual/MotoGP25/P/Content/Paks");
    let result = w.service().inspect_root(Path::new("manual/MotoGP25"));
    assert!(result.is_ok());
}

/// Runs only `game-detection.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = GameWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("game-detection.feature"))
        .await;
    !writer.execution_has_failed()
}
