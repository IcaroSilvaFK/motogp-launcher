//! Steps for `tests/features/catalog-install.feature` (issue #74), with
//! in-memory fakes of the download, staging and library ports.
use crate::install_fakes::{downloadable, Behavior, Rig};
use crate::library_fakes::{fake_hash, new_state, put_file, service, Shared};
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{CancelFlag, CatalogInstallError, ModService};
use motogp_domain::ModEntry;

const PAK: &[u8] = b"AAAAAA-pak";
const ID: &str = "rebalanced-ai";

#[derive(World)]
#[world(init = Self::new)]
struct InstallWorld {
    rig: Option<Rig>,
    lib: Shared,
    active: bool,
    cancelling: bool,
    result: Option<Result<Vec<ModEntry>, CatalogInstallError>>,
}

impl std::fmt::Debug for InstallWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstallWorld")
    }
}

impl InstallWorld {
    fn new() -> Self {
        Self {
            rig: None,
            lib: new_state(),
            active: false,
            cancelling: false,
            result: None,
        }
    }

    fn rig(&mut self) -> &Rig {
        let active = self.active;
        self.rig.get_or_insert_with(|| {
            let rig = Rig::matching(PAK);
            rig.session.set_active(active);
            rig
        })
    }

    fn with_entry(&mut self, size: u64, sha256: &str) {
        self.rig = Some(Rig::new(
            vec![downloadable(
                ID,
                "https://example.com/files/rebalanced.pak",
                size,
                sha256,
            )],
            PAK,
        ));
    }

    fn install(&mut self, id: &str) {
        let cancelling = self.cancelling;
        let lib = self.lib.clone();
        let rig = self.rig();
        if cancelling {
            rig.downloader.behave(Behavior::CancelMidway);
        }
        let outcome = rig.installer.prepare(
            &rig.catalog,
            &rig.profiles(),
            id,
            &mut |_| {},
            &CancelFlag::new(),
        );
        let result = outcome.and_then(|staged| {
            // The library fake has its own storage: mirror the staged file into it.
            let content = rig.disk.lock().unwrap().files[staged.path()].clone();
            put_file(&lib, &staged.path().to_string_lossy(), &content);
            let mods: ModService<_, _, _, _> = service(&lib);
            rig.installer
                .install(&mods, &rig.profiles(), staged, None)
                .map(|o| o.mods)
        });
        self.result = Some(result);
    }

    fn error(&self) -> &CatalogInstallError {
        self.result
            .as_ref()
            .expect("install not run")
            .as_ref()
            .expect_err("expected a failure")
    }
}

#[given(expr = "the catalog has the downloadable entry {string} with a matching file")]
fn matching(w: &mut InstallWorld, _id: String) {
    w.rig = Some(Rig::matching(PAK));
}

#[given(expr = "the catalog entry {string} declares a size different from the file")]
fn other_size(w: &mut InstallWorld, _id: String) {
    w.with_entry(PAK.len() as u64 + 5, &fake_hash(PAK));
}

#[given(expr = "the catalog entry {string} declares another SHA-256")]
fn other_hash(w: &mut InstallWorld, _id: String) {
    w.with_entry(PAK.len() as u64, &"0".repeat(64));
}

#[given(expr = "the catalog entry {string} has no download")]
fn page_only(w: &mut InstallWorld, id: String) {
    w.rig = Some(Rig::new(vec![crate::catalog_fakes::raw(&id)], PAK));
}

#[given("a game session is active")]
fn session_active(w: &mut InstallWorld) {
    w.active = true;
    w.rig().session.set_active(true);
}

#[given(expr = "an install of {string} is downloading")]
fn downloading(w: &mut InstallWorld, _id: String) {
    w.cancelling = true;
}

#[when(expr = "the user installs {string}")]
fn installs(w: &mut InstallWorld, id: String) {
    w.install(&id);
}

#[when("the user cancels it")]
fn cancels(w: &mut InstallWorld) {
    w.install(ID);
}

#[then("the mod appears in the library")]
fn appears(w: &mut InstallWorld) {
    let entries = w
        .result
        .clone()
        .expect("install not run")
        .expect("install failed");
    assert_eq!(entries.len(), 1);
    assert_eq!(w.lib.borrow().index, entries);
}

#[then("no staging files remain")]
fn no_staging(w: &mut InstallWorld) {
    assert!(w.rig().disk_is_empty());
}

#[then("the install fails with a size mismatch")]
fn size_mismatch(w: &mut InstallWorld) {
    assert!(matches!(
        w.error(),
        CatalogInstallError::SizeMismatch { .. }
    ));
}

#[then("the install fails with a hash mismatch")]
fn hash_mismatch(w: &mut InstallWorld) {
    assert_eq!(w.error(), &CatalogInstallError::HashMismatch);
}

#[then("the library is unchanged and no staging files remain")]
fn unchanged(w: &mut InstallWorld) {
    assert!(w.lib.borrow().index.is_empty());
    assert!(w.rig().disk_is_empty());
}

#[then("the install fails as not downloadable")]
fn not_downloadable(w: &mut InstallWorld) {
    assert!(matches!(
        w.error(),
        CatalogInstallError::NotDownloadable { .. }
    ));
}

#[then("nothing is downloaded")]
fn nothing_downloaded(w: &mut InstallWorld) {
    assert_eq!(w.rig().downloader.calls(), 0);
    assert!(w.rig().disk_is_empty());
}

#[then("the install fails because the library is locked by the session")]
fn locked(w: &mut InstallWorld) {
    assert_eq!(w.error(), &CatalogInstallError::SessionActive);
}

#[then("the install fails as cancelled")]
fn cancelled(w: &mut InstallWorld) {
    assert_eq!(w.error(), &CatalogInstallError::Cancelled);
}

/// Runs only `catalog-install.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = InstallWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("catalog-install.feature"))
        .await;
    !writer.execution_has_failed()
}
