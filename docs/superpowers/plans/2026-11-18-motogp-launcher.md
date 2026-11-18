# MotoGP Mod Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A lightweight desktop launcher (Tauri) that imports `.pak` mods for the MotoGP games, organizes them into profiles and deploys/removes the mods in `~mods` around each game session.

**Architecture:** The Rust backend (`src-tauri/src`) has one module per responsibility: store (JSON), library, profiles, game, deploy and launch. Each module has unit tests with `tempfile`. The `#[tauri::command]`s stay thin in `commands.rs`. The React+TS frontend only calls the commands via `invoke`, with no business logic.

**Tech Stack:** Tauri 2, stable Rust (MSVC), crates `serde`, `serde_json`, `thiserror`, `sha2`, `zip`, `sysinfo`, `winreg`, `tempfile` (dev); plugins `tauri-plugin-dialog` and `tauri-plugin-opener`; React 18 + TypeScript + Vite; pnpm.

**Spec:** `docs/superpowers/specs/2026-11-18-motogp-launcher-design.md`

## Global Constraints

- Everything runs on **native Windows 10/11** (PowerShell), not WSL. Project root: `C:\Users\whyy\dev\motogp-launcher`.
- Never delete a file in `~mods` that is not listed in `deployed.json`.
- No fixed Steam App ID in the code; it comes from `appmanifest_*.acf`.
- Deployed name: `format!("{:03}_{}_{}", index, mod_id, original_file_name)`.
- Mod id: first 12 (lowercase) hex characters of the `.pak` SHA-256.
- Watcher: poll every 2 s; `NeverStarted` after 120 s without seeing the process; `Ended` after 2 polls in a row without the process.
- Release executable < 15 MB.
- Do not copy SwoftyLauncher code (PolyForm Noncommercial).
- UI texts and error messages in English.

## Review Focus

1. **Launcher closes/crashes during the session:** when reopened, it cleans `~mods` using `deployed.json`. Test in Task 6 (`deploy_cleans_previous_session_first`) + recovery in `setup()` (Task 8).
2. **User files in `~mods`:** never deleted nor overwritten. Tests `clean_removes_only_deployed_files` and `deploy_refuses_to_overwrite_foreign_file` (Task 6).
3. **Paths with spaces and accents** (`C:\Users\Usuário Padrão\…`, `Program Files (x86)`): tests `roundtrip_in_path_with_accents` (Task 2) and `parse_library_folders_unescapes_backslashes` (Task 5).
4. **Malformed zip:** nested folders, no `.pak` or an entry with `..`. Tests `import_zip_with_nested_folders_finds_all_paks`, `import_zip_without_pak_errors` and `zip_entry_with_parent_traversal_is_ignored` (Task 3).
5. **Corrupted JSON** (e.g. a hand-edited `profiles.json`): returns an error instead of silently resetting the profiles. Test `corrupted_json_is_an_error` (Task 2).

---

### Task 1: Prerequisites and scaffold

**Files:**
- Create: the whole project via `create-tauri-app`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Produces: lib crate `motogp_launcher_lib`; `cargo test --manifest-path src-tauri/Cargo.toml` working.

- [ ] **Step 1: Install the tools (PowerShell as a normal user)**

```powershell
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id OpenJS.NodeJS.LTS -e
# close and reopen the terminal
rustup default stable-msvc
npm install -g pnpm
```

Verify: `rustc --version`, `cargo --version`, `node --version` and `pnpm --version` print versions. WebView2 already ships with up-to-date Windows 10/11.

- [ ] **Step 2: Generate the project without losing `docs/`**

```powershell
mkdir C:\Users\whyy\dev\_scaffold; cd C:\Users\whyy\dev\_scaffold
pnpm create tauri-app motogp-launcher --template react-ts --manager pnpm --identifier dev.motogplauncher.app --yes
Move-Item C:\Users\whyy\dev\motogp-launcher\docs .\motogp-launcher\
Remove-Item C:\Users\whyy\dev\motogp-launcher -Recurse
Move-Item .\motogp-launcher C:\Users\whyy\dev\
cd C:\Users\whyy\dev; Remove-Item _scaffold; cd motogp-launcher
pnpm install
```

If your version of `create-tauri-app` does not accept `--yes`, answer the prompts with the same values.

- [ ] **Step 3: Add dependencies**

```powershell
cd src-tauri
cargo add serde --features derive
cargo add serde_json thiserror sha2 zip sysinfo
cargo add winreg --target "cfg(windows)"
cargo add --dev tempfile
cd ..
pnpm tauri add dialog
pnpm tauri add opener   # skip if the template already ships it
```

Confirm that `src-tauri/Cargo.toml` has `[lib] name = "motogp_launcher_lib"`; if the generated name is different, use the generated one throughout the plan.

- [ ] **Step 4: Adjust the window** in `tauri.conf.json`: `productName` = `"MotoGP Launcher"` and a `1100x700` window with `minWidth 900` and `minHeight 600`.

- [ ] **Step 5: Verify**

Run: `pnpm tauri dev` → opens the template window. Close it.
Run: `cargo test --manifest-path src-tauri/Cargo.toml` → `test result: ok. 0 passed`.

- [ ] **Step 6: Commit**

```powershell
git init; git add -A; git commit -m "chore: scaffold tauri react-ts app"
```

---

### Task 2: Errors and JSON persistence

**Files:**
- Create: `src-tauri/src/error.rs`, `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod error; mod store;`)

**Interfaces:**
- Produces:
  - `pub enum AppError { Io(std::io::Error), Json(serde_json::Error), Zip(zip::result::ZipError), Msg(String) }` (`thiserror`, `#[from]` on the first three); `impl serde::Serialize for AppError` serializes as `self.to_string()`; `pub type Result<T> = std::result::Result<T, AppError>`; `pub fn msg(s: impl Into<String>) -> AppError`.
  - `pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T>`: if the file does not exist, returns `T::default()`; if it exists and is invalid, returns `Err`.
  - `pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()>`: creates the parent folders, writes to `<path>.tmp` and does a `rename` (atomic write), with pretty-print.
  - `#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)] pub struct Config { pub game_dir: Option<PathBuf>, pub active_profile: Option<String> }`
  - `#[derive(Clone)] pub struct DataDir(pub PathBuf)` with `config()`, `profiles()`, `library_index()`, `deployed()` (→ `<root>/config.json`, `profiles.json`, `library.json`, `deployed.json`) and `library_dir()` (→ `<root>/library`), all returning `PathBuf`.

- [ ] **Step 1: Tests in `store.rs` (`#[cfg(test)] mod tests`)**

```rust
#[test] fn missing_file_returns_default() {
    let d = tempfile::tempdir().unwrap();
    let c: Config = load_json(&d.path().join("nope.json")).unwrap();
    assert_eq!(c, Config::default());
}
#[test] fn roundtrip_in_path_with_accents() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("Usuário Padrão").join("config.json");
    let c = Config { game_dir: Some("C:\\Program Files (x86)\\Steam\\steamapps\\common\\MotoGP25".into()), active_profile: Some("p1".into()) };
    save_json(&p, &c).unwrap();
    assert_eq!(load_json::<Config>(&p).unwrap(), c);
    assert!(!p.with_extension("json.tmp").exists());
}
#[test] fn corrupted_json_is_an_error() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("config.json");
    std::fs::write(&p, "{ not json").unwrap();
    assert!(load_json::<Config>(&p).is_err());
}
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml store` → FAIL (does not compile).
- [ ] **Step 3:** Implement `error.rs` and `store.rs` per Interfaces.
- [ ] **Step 4:** Same command → 3 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: json store and error type"`

---

### Task 3: Mod library

**Files:**
- Create: `src-tauri/src/library.rs`
- Modify: `src-tauri/src/lib.rs` (`mod library;`)

**Interfaces:**
- Consumes: `DataDir`, `load_json`, `save_json`, `Result`, `msg` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct ModEntry { pub id: String, pub name: String, pub files: Vec<String>, pub size_bytes: u64 }`. `files` holds the original names (e.g. `["Ducati_P.pak","Ducati_P.utoc","Ducati_P.ucas"]`) and `name` is the base name without the trailing `_P` (e.g. `"Ducati"`).
  - `pub struct Library { dir: PathBuf, index: PathBuf }`
  - `impl Library { pub fn open(data: &DataDir) -> Self; pub fn list(&self) -> Result<Vec<ModEntry>>; pub fn import(&self, source: &Path) -> Result<Vec<ModEntry>>; pub fn remove(&self, id: &str) -> Result<()>; pub fn mod_dir(&self, id: &str) -> PathBuf }`
  - `pub fn group_pak_files(names: &[String]) -> Result<Vec<(String, Vec<String>)>>`: groups by base name (case-insensitive extension), accepts only `.pak/.utoc/.ucas` and ignores the rest. If a group has no `.pak`, returns `Err("'<stem>' has no .pak file")`.
  - `fn mod_id(pak: &Path) -> Result<String>`: streaming SHA-256, 12 hex.

Behavior of `import`:
- If `source` is `.zip`: extracts to a temporary folder inside `dir`, ignoring entries without `enclosed_name()` and flattening subfolders. If the zip has no `.pak`: `Err("no .pak found in the zip")`.
- If `source` is `.pak/.utoc/.ucas`: uses the siblings with the same base name in the same folder.
- Any other extension: `Err("unsupported format: <ext>")`.
- Each group is copied to `mod_dir(id)`. If the `id` is already in the index, it does not duplicate: returns the existing entry.

- [ ] **Step 1: Tests.** Create a helper `fn write(dir, name, bytes)` and a helper that builds a zip with `zip::ZipWriter`.

```rust
#[test] fn import_single_pak_copies_into_library()      // 1 entry; files == ["A.pak"]; mod_dir(id)/A.pak exists; id.len()==12
#[test] fn import_pak_picks_up_utoc_ucas_siblings()     // A_P.pak/.utoc/.ucas + B.pak in the folder → importing A_P.pak gives the files of A_P (3), name=="A"; B does not get in
#[test] fn import_zip_with_nested_folders_finds_all_paks() // zip with "x/y/A.pak" and "B.pak" → 2 entries
#[test] fn import_zip_without_pak_errors()               // zip with only readme.txt → Err containing "no .pak"
#[test] fn utoc_without_pak_errors()                     // group_pak_files(["A.utoc"]) → Err containing "has no .pak file"
#[test] fn reimport_same_pak_does_not_duplicate()        // imports 2x → list().len()==1
#[test] fn remove_deletes_dir_and_index_entry()          // after remove: list() empty and mod_dir does not exist
#[test] fn zip_entry_with_parent_traversal_is_ignored()  // zip with "../evil.pak" and "ok.pak" → 1 entry (ok) and nothing outside dir
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml library` → FAIL.
- [ ] **Step 3:** Implement per Interfaces.
- [ ] **Step 4:** Same command → 8 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: mod library import (pak, utoc/ucas, zip)"`

---

### Task 4: Profiles

**Files:**
- Create: `src-tauri/src/profiles.rs`
- Modify: `src-tauri/src/lib.rs` (`mod profiles;`)

**Interfaces:**
- Consumes: `DataDir`, `load_json`, `save_json` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct Profile { pub id: String, pub name: String, pub mod_ids: Vec<String> }`
  - `pub struct Profiles { path: PathBuf }`
  - `impl Profiles { pub fn open(data: &DataDir) -> Self; pub fn list(&self) -> Result<Vec<Profile>>; pub fn create(&self, name: &str) -> Result<Profile>; pub fn rename(&self, id: &str, name: &str) -> Result<()>; pub fn delete(&self, id: &str) -> Result<()>; pub fn set_mods(&self, id: &str, mod_ids: Vec<String>) -> Result<()>; pub fn get(&self, id: &str) -> Result<Profile>; pub fn remove_mod_everywhere(&self, mod_id: &str) -> Result<()> }`
  - If the file does not exist or is empty, `list()` creates and saves `Profile { id: "vanilla", name: "Vanilla", mod_ids: [] }`. The id of new profiles is `format!("p{}", unix_millis)`. Empty or whitespace-only name: `Err("empty profile name")`. Unknown id: `Err("profile not found")`.

- [ ] **Step 1: Tests**

```rust
#[test] fn empty_store_seeds_vanilla()           // list() == [Vanilla]
#[test] fn create_then_list()                    // create("Pinturas 2025") → list().len()==2, name matches
#[test] fn set_mods_preserves_order()            // set_mods(id, ["b","a","c"]) → get(id).mod_ids == ["b","a","c"]
#[test] fn delete_removes_profile()
#[test] fn remove_mod_everywhere_strips_from_all_profiles()
#[test] fn blank_name_is_rejected()              // create("  ") is_err
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml profiles` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** → 6 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: profiles store"`

---

### Task 5: Game detection

**Files:**
- Create: `src-tauri/src/game.rs`
- Modify: `src-tauri/src/lib.rs` (`mod game;`)

**Interfaces:**
- Consumes: `Result`, `msg` (Task 2).
- Produces:
  - `#[derive(Serialize, Clone, Debug, PartialEq)] pub struct GameInfo { pub name: String, pub root: PathBuf, pub mods_dir: PathBuf, pub app_id: Option<u32> }`
  - `pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf>`: takes the value of each `"path"  "<value>"` line and replaces `\\` with `\`.
  - `pub fn parse_app_manifest(acf: &str) -> Option<(u32, String)>`: reads `"appid"` and `"installdir"`.
  - `pub fn find_paks_dir(root: &Path) -> Option<PathBuf>`: first immediate subfolder `X` of `root` in which `X/Content/Paks` exists (does not consider the `Engine` folder). Returns `X/Content/Paks`.
  - `pub fn inspect_game(root: &Path) -> Result<GameInfo>`: `mods_dir = paks/"~mods"` (does not create the folder). The `app_id` comes from the `appmanifest_*.acf` in `root/../..` whose `installdir` equals the name of the `root` folder (case-insensitive). Without `Content/Paks`: `Err("folder does not look like an Unreal game: Content/Paks not found")`.
  - `pub fn detect_games(steam_root: &Path) -> Vec<GameInfo>`: reads `steam_root/steamapps/libraryfolders.vdf` and, for each library, the folders in `steamapps/common` whose name starts with `MotoGP` (case-insensitive) and that pass `inspect_game`.
  - `#[cfg(windows)] pub fn steam_root_from_registry() -> Option<PathBuf>`: `HKCU\Software\Valve\Steam`, value `SteamPath`.

- [ ] **Step 1: Tests** (the fixtures build a fake Steam tree in a tempdir)

```rust
#[test] fn parse_library_folders_unescapes_backslashes() {
    let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
    assert_eq!(parse_library_folders(vdf), vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]);
}
#[test] fn parse_app_manifest_reads_id_and_installdir() // "\"appid\"\t\t\"123\"" + "\"installdir\"\t\t\"MotoGP25\"" → Some((123,"MotoGP25"))
#[test] fn inspect_game_finds_mods_dir_and_app_id()     // common/MotoGP25/MotoGP25/Content/Paks + steamapps/appmanifest_123.acf → mods_dir ends in "Content\\Paks\\~mods", app_id Some(123)
#[test] fn inspect_game_without_paks_errors()
#[test] fn detect_games_only_returns_motogp_folders()   // common/MotoGP25 (valid) + common/OtherGame (valid) → only MotoGP25
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml game` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** → 5 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: steam game detection"`

---

### Task 6: Deploy and clean mods

**Files:**
- Create: `src-tauri/src/deploy.rs`
- Modify: `src-tauri/src/lib.rs` (`mod deploy;`)

**Interfaces:**
- Consumes: `Library::mod_dir`, `ModEntry` (Task 3); `load_json`, `save_json` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Default, Debug, PartialEq)] pub struct Deployed { pub mods_dir: Option<PathBuf>, pub files: Vec<String> }`
  - `pub fn deployed_name(index: usize, mod_id: &str, original: &str) -> String`
  - `pub fn deploy(lib: &Library, mods: &[ModEntry], mods_dir: &Path, manifest: &Path) -> Result<Deployed>`: first calls `clean(manifest)`. With empty `mods`, returns without creating the folder. Otherwise does `create_dir_all(mods_dir)` and, for each file (the `index` is the position of the mod in `mods`), tries `fs::hard_link` and falls back to `fs::copy`. If the destination already exists: `Err("file already exists in ~mods: <name>")`. The manifest is saved **after each file**, so a crash in the middle can still be cleaned.
  - `pub fn clean(manifest: &Path) -> Result<usize>`: deletes each listed `mods_dir/file` (ignores `NotFound`), saves `Deployed::default()` and returns how many it deleted.

- [ ] **Step 1: Tests**

```rust
#[test] fn deploy_writes_files_in_profile_order_with_prefix() // mods [B, A] → "000_<idB>_B.pak", "001_<idA>_A.pak"
#[test] fn deploy_preserves_p_suffix()                        // original "Ducati_P.pak" → name ends with "_Ducati_P.pak"
#[test] fn clean_removes_only_deployed_files()                // manual "meu_mod.pak" in ~mods is still there after clean
#[test] fn clean_tolerates_already_missing_file()             // delete one by hand beforehand → clean Ok
#[test] fn deploy_cleans_previous_session_first()             // deploy [A]; deploy [B] → only B's files in ~mods
#[test] fn deploy_refuses_to_overwrite_foreign_file()         // create the target name by hand → Err containing "already exists"; manual file intact
#[test] fn deploy_empty_profile_leaves_dir_untouched()        // mods [] → ~mods is not created
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml deploy` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** → 7 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: deploy and clean mods with manifest"`

---

### Task 7: Launch the game and track the session

**Files:**
- Create: `src-tauri/src/launch.rs`
- Modify: `src-tauri/src/lib.rs` (`mod launch;`)

**Interfaces:**
- Consumes: `GameInfo` (Task 5).
- Produces:
  - `pub fn launch_url(game: &GameInfo) -> Result<String>` → `"steam://rungameid/{app_id}"`; without `app_id`: `Err("game App ID not found; open the game from Steam once")`.
  - `#[derive(Debug, PartialEq, Clone, Copy)] pub enum WatchEvent { Started, Ended, NeverStarted }`
  - `#[derive(Default)] pub struct WatchState { seen: bool, missing_polls: u32 }`
  - `pub fn watch_step(s: &mut WatchState, running: bool, elapsed: Duration) -> Option<WatchEvent>`: pure state machine. The first time `running` → `Started`. Once seen, 2 polls in a row not running → `Ended`; a running poll resets the counter. Never seen and `elapsed >= 120s` → `NeverStarted`.
  - `pub fn is_game_running(sys: &mut sysinfo::System, root: &Path) -> bool`: refreshes the processes and checks whether any `exe()` starts with `root` (case-insensitive comparison, since it is Windows).
  - `pub const POLL: Duration = Duration::from_secs(2);`

- [ ] **Step 1: Tests**

```rust
#[test] fn launch_url_uses_app_id()                 // app_id Some(123) → "steam://rungameid/123"
#[test] fn launch_url_without_app_id_errors()
#[test] fn started_then_ended_after_two_missing_polls() // true→Started; false→None; false→Ended
#[test] fn flapping_resets_missing_counter()        // true, false, true, false → no Ended
#[test] fn never_started_after_120s()               // false @119s → None; false @120s → NeverStarted
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml launch` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** → 5 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: launch url and session watcher"`

---

### Task 8: Tauri commands and recovery on startup

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (register plugins, state, commands and `setup`)

**Interfaces:**
- Consumes: everything from Tasks 2–7.
- Produces (names the frontend calls via `invoke`, with camelCase arguments on the JS side):
  - `pub struct AppState { pub data: DataDir, pub session_active: Arc<AtomicBool> }`
  - `get_config() -> Config`
  - `detect_games() -> Vec<GameInfo>`: on Windows, `steam_root_from_registry()` + `detect_games`; empty if Steam is not found.
  - `set_game_dir(path: String) -> GameInfo`: `inspect_game`, then writes `config.game_dir`.
  - `get_game() -> Option<GameInfo>`: `inspect_game(config.game_dir)`, `None` if there is none.
  - `list_mods() -> Vec<ModEntry>`; `import_mods(paths: Vec<String>) -> Vec<ModEntry>` (concatenates the results; on the first failure returns the error); `remove_mod(id: String)` (also calls `remove_mod_everywhere`).
  - `list_profiles() -> Vec<Profile>`; `create_profile(name) -> Profile`; `rename_profile(id, name)`; `delete_profile(id)`; `set_profile_mods(id, modIds: Vec<String>)`; `set_active_profile(id)`.
  - `play(app: AppHandle, profileId: String) -> ()`: if `session_active` is already true, `Err("the game is already running")`. Otherwise: `get_game` (or `Err("choose the game folder")`), resolves the profile's `ModEntry`s in order, ignoring ids that are no longer in the library, calls `deploy` and opens `launch_url` with `tauri_plugin_opener::OpenerExt::opener().open_url`. Then creates a thread that polls with `POLL` / `watch_step` / `is_game_running`. On `Ended` or `NeverStarted`, calls `clean`, resets `session_active` and emits `app.emit("session", SessionEvent)`. On `Started`, only emits.
  - `#[derive(Serialize, Clone)] pub struct SessionEvent { pub state: &'static str /* "started" | "ended" | "never-started" */, pub cleaned: usize }`
- `setup()` in `lib.rs`: `data = DataDir(app.path().app_data_dir()?)`. Calls `clean(data.deployed())`; if it returns > 0, does an `eprintln!` with the number (this is the crash recovery). Then registers the `AppState`.

- [ ] **Step 1: Test of the extracted pure rule** in `commands.rs`: `pub fn resolve_profile_mods(profile: &Profile, library: &[ModEntry]) -> Vec<ModEntry>`.

```rust
#[test] fn resolve_profile_mods_keeps_order_and_skips_missing() // mod_ids ["b","gone","a"] → [b, a]
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml commands` → FAIL.
- [ ] **Step 3:** Implement `commands.rs` and the wiring in `lib.rs` (`.plugin(tauri_plugin_dialog::init())`, `.plugin(tauri_plugin_opener::init())`, `.invoke_handler(tauri::generate_handler![...])`). Check that `capabilities/default.json` has `dialog:default` and `opener:default`; to open `steam://`, add the permission `opener:allow-open-url` with `{ "url": "steam://*" }`.
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml` → all pass (≥ 35). `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` → no warnings.
- [ ] **Step 5:** `git add -A; git commit -m "feat: tauri commands, session thread, crash recovery"`

---

### Task 9: Interface

**Files:**
- Create: `src/api.ts`, `src/pages/Library.tsx`, `src/pages/Profiles.tsx`, `src/pages/Settings.tsx`
- Modify: `src/App.tsx`, `src/App.css` (remove the template content)

**Interfaces:**
- Consumes: commands and the `"session"` event from Task 8.
- Produces: in `src/api.ts`, the types `Config`, `GameInfo`, `ModEntry`, `Profile` and `SessionEvent`, mirroring the Rust in snake_case (`game_dir`, `mods_dir`, `app_id`, `mod_ids`, `size_bytes`, `active_profile`), and one typed function per command (`listMods()`, `importMods(paths)`, `play(profileId)` etc.) using `invoke` from `@tauri-apps/api/core`.

Screens:
- **Layout:** sidebar (Library, Profiles, Settings). Below it, the active profile selector and the **Play** button, disabled without a configured game or with an active session. The session state comes from `listen("session", …)`: "Running…" / "Session ended — N files removed" / "The game did not open within 2 min; mods removed".
- **Library:** lists name, size and number of files, with a remove button. The "Import" button opens `open({ multiple: true, filters: [{ name: "Mods", extensions: ["pak","utoc","ucas","zip"] }] })` from `@tauri-apps/plugin-dialog`. Drag and drop uses `getCurrentWebview().onDragDropEvent` (`drop` event → `importMods(paths)`).
- **Profiles:** list of profiles (create/rename/delete; "vanilla" cannot be deleted). The selected profile shows checkboxes for the mods and ↑/↓ buttons for the order, and saves with `setProfileMods`.
- **Settings:** shows the current game (name, `mods_dir`, `app_id`), the `detectGames()` list with a "Use this" button and "Choose folder…" (`open({ directory: true })` → `setGameDir`).
- Every rejected `invoke` shows the error message in a notice at the top.

- [ ] **Step 1:** `pnpm tsc --noEmit` → no errors.
- [ ] **Step 2: Manual verification** with `pnpm tauri dev`, without the game, using a fake folder `C:\temp\MotoGPFake\MotoGPFake\Content\Paks`:
  - Settings → choose `C:\temp\MotoGPFake` → shows the `mods_dir`, and an empty `app_id`.
  - Import any `.pak` (create one with `fsutil file createnew C:\temp\teste.pak 1024`) → it appears in the Library; importing again does not duplicate.
  - Create the profile "Teste" and check the mod.
  - Play → notice "game App ID not found…" and nothing created in `~mods`.
- [ ] **Step 3:** `git add -A; git commit -m "feat: launcher UI"`

---

### Task 10: Build and real test

- [ ] **Step 1:** `pnpm tauri build` → produces `src-tauri\target\release\motogp-launcher.exe` and the installer in `src-tauri\target\release\bundle\`.
- [ ] **Step 2:** `(Get-Item src-tauri\target\release\motogp-launcher.exe).Length / 1MB` → < 15.
- [ ] **Step 3: Test with the real game**
  - Settings → Detect → the installed MotoGP appears, with `app_id` filled in.
  - Import a real mod (e.g. a livery) → profile "Teste" → Play → the mod shows up in the game.
  - During the game, `~mods` has the `000_…` files. Close the game → within ~4 s the "Session ended" notice appears and `~mods` goes back to what it was before.
  - Put a `.pak` of your own in `~mods` by hand, run a session and confirm it is still there.
  - Crash: Play, kill the launcher via Task Manager, close the game and open the launcher → `~mods` is clean.
- [ ] **Step 4:** `git add -A; git commit -m "chore: release build verified"`
