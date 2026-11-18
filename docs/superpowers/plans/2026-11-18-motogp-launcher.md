# MotoGP Mod Launcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Launcher desktop leve (Tauri) que importa mods `.pak` dos jogos MotoGP, organiza em perfis e aplica/remove os mods em `~mods` ao redor de cada sessão de jogo.

**Architecture:** O backend Rust (`src-tauri/src`) tem um módulo por responsabilidade: store (JSON), library, profiles, game, deploy e launch. Cada módulo tem testes unitários com `tempfile`. Os `#[tauri::command]` ficam finos em `commands.rs`. O frontend React+TS só chama os comandos via `invoke`, sem lógica de negócio.

**Tech Stack:** Tauri 2, Rust estável (MSVC), crates `serde`, `serde_json`, `thiserror`, `sha2`, `zip`, `sysinfo`, `winreg`, `tempfile` (dev); plugins `tauri-plugin-dialog` e `tauri-plugin-opener`; React 18 + TypeScript + Vite; pnpm.

**Spec:** `docs/superpowers/specs/2026-11-18-motogp-launcher-design.md`

## Global Constraints

- Tudo roda no **Windows 10/11 nativo** (PowerShell), não no WSL. Raiz do projeto: `C:\Users\whyy\dev\motogp-launcher`.
- Nunca apagar arquivo em `~mods` que não esteja listado em `deployed.json`.
- Nenhum Steam App ID fixo no código; ele vem do `appmanifest_*.acf`.
- Nome aplicado: `format!("{:03}_{}_{}", index, mod_id, original_file_name)`.
- Id do mod: 12 primeiros caracteres hex (minúsculos) do SHA-256 do `.pak`.
- Watcher: poll a cada 2 s; `NeverStarted` após 120 s sem ver o processo; `Ended` após 2 polls seguidos sem o processo.
- Executável release < 15 MB.
- Não copiar código do SwoftyLauncher (PolyForm Noncommercial).
- Textos da UI e mensagens de erro em português.

## Review Focus

1. **Launcher fecha/crasha durante a sessão:** ao abrir de novo, ele limpa `~mods` usando `deployed.json`. Teste em Task 6 (`deploy_cleans_previous_session_first`) + recuperação em `setup()` (Task 8).
2. **Arquivos do usuário em `~mods`:** nunca são apagados nem sobrescritos. Testes `clean_removes_only_deployed_files` e `deploy_refuses_to_overwrite_foreign_file` (Task 6).
3. **Caminhos com espaço e acento** (`C:\Users\Usuário Padrão\…`, `Program Files (x86)`): testes `roundtrip_in_path_with_accents` (Task 2) e `parse_library_folders_unescapes_backslashes` (Task 5).
4. **Zip malformado:** pastas aninhadas, nenhum `.pak` ou entrada com `..`. Testes `import_zip_with_nested_folders_finds_all_paks`, `import_zip_without_pak_errors` e `zip_entry_with_parent_traversal_is_ignored` (Task 3).
5. **JSON corrompido** (ex.: `profiles.json` editado à mão): retorna erro em vez de zerar os perfis em silêncio. Teste `corrupted_json_is_an_error` (Task 2).

---

### Task 1: Pré-requisitos e scaffold

**Files:**
- Create: projeto inteiro via `create-tauri-app`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Produces: crate lib `motogp_launcher_lib`; `cargo test --manifest-path src-tauri/Cargo.toml` funcionando.

- [ ] **Step 1: Instalar as ferramentas (PowerShell como usuário normal)**

```powershell
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id OpenJS.NodeJS.LTS -e
# feche e reabra o terminal
rustup default stable-msvc
npm install -g pnpm
```

Verify: `rustc --version`, `cargo --version`, `node --version` e `pnpm --version` imprimem versões. O WebView2 já vem no Windows 10/11 atualizado.

- [ ] **Step 2: Gerar o projeto sem perder `docs/`**

```powershell
mkdir C:\Users\whyy\dev\_scaffold; cd C:\Users\whyy\dev\_scaffold
pnpm create tauri-app motogp-launcher --template react-ts --manager pnpm --identifier dev.motogplauncher.app --yes
Move-Item C:\Users\whyy\dev\motogp-launcher\docs .\motogp-launcher\
Remove-Item C:\Users\whyy\dev\motogp-launcher -Recurse
Move-Item .\motogp-launcher C:\Users\whyy\dev\
cd C:\Users\whyy\dev; Remove-Item _scaffold; cd motogp-launcher
pnpm install
```

Se a sua versão do `create-tauri-app` não aceitar `--yes`, responda os prompts com os mesmos valores.

- [ ] **Step 3: Adicionar dependências**

```powershell
cd src-tauri
cargo add serde --features derive
cargo add serde_json thiserror sha2 zip sysinfo
cargo add winreg --target "cfg(windows)"
cargo add --dev tempfile
cd ..
pnpm tauri add dialog
pnpm tauri add opener   # pule se o template já trouxer
```

Confirme que `src-tauri/Cargo.toml` tem `[lib] name = "motogp_launcher_lib"`; se o nome gerado for outro, use o gerado em todo o plano.

- [ ] **Step 4: Ajustar a janela** em `tauri.conf.json`: `productName` = `"MotoGP Launcher"` e janela `1100x700` com `minWidth 900` e `minHeight 600`.

- [ ] **Step 5: Verificar**

Run: `pnpm tauri dev` → abre a janela do template. Feche.
Run: `cargo test --manifest-path src-tauri/Cargo.toml` → `test result: ok. 0 passed`.

- [ ] **Step 6: Commit**

```powershell
git init; git add -A; git commit -m "chore: scaffold tauri react-ts app"
```

---

### Task 2: Erros e persistência JSON

**Files:**
- Create: `src-tauri/src/error.rs`, `src-tauri/src/store.rs`
- Modify: `src-tauri/src/lib.rs` (adicionar `mod error; mod store;`)

**Interfaces:**
- Produces:
  - `pub enum AppError { Io(std::io::Error), Json(serde_json::Error), Zip(zip::result::ZipError), Msg(String) }` (`thiserror`, `#[from]` nos três primeiros); `impl serde::Serialize for AppError` serializa como `self.to_string()`; `pub type Result<T> = std::result::Result<T, AppError>`; `pub fn msg(s: impl Into<String>) -> AppError`.
  - `pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> Result<T>`: se o arquivo não existe, retorna `T::default()`; se existe e é inválido, retorna `Err`.
  - `pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()>`: cria as pastas pai, grava em `<path>.tmp` e faz `rename` (escrita atômica), com pretty-print.
  - `#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)] pub struct Config { pub game_dir: Option<PathBuf>, pub active_profile: Option<String> }`
  - `#[derive(Clone)] pub struct DataDir(pub PathBuf)` com `config()`, `profiles()`, `library_index()`, `deployed()` (→ `<root>/config.json`, `profiles.json`, `library.json`, `deployed.json`) e `library_dir()` (→ `<root>/library`), todos retornando `PathBuf`.

- [ ] **Step 1: Testes em `store.rs` (`#[cfg(test)] mod tests`)**

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

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml store` → FAIL (não compila).
- [ ] **Step 3:** Implementar `error.rs` e `store.rs` conforme Interfaces.
- [ ] **Step 4:** Mesmo comando → 3 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: json store and error type"`

---

### Task 3: Biblioteca de mods

**Files:**
- Create: `src-tauri/src/library.rs`
- Modify: `src-tauri/src/lib.rs` (`mod library;`)

**Interfaces:**
- Consumes: `DataDir`, `load_json`, `save_json`, `Result`, `msg` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct ModEntry { pub id: String, pub name: String, pub files: Vec<String>, pub size_bytes: u64 }`. `files` guarda os nomes originais (ex.: `["Ducati_P.pak","Ducati_P.utoc","Ducati_P.ucas"]`) e `name` é o nome-base sem o `_P` final (ex.: `"Ducati"`).
  - `pub struct Library { dir: PathBuf, index: PathBuf }`
  - `impl Library { pub fn open(data: &DataDir) -> Self; pub fn list(&self) -> Result<Vec<ModEntry>>; pub fn import(&self, source: &Path) -> Result<Vec<ModEntry>>; pub fn remove(&self, id: &str) -> Result<()>; pub fn mod_dir(&self, id: &str) -> PathBuf }`
  - `pub fn group_pak_files(names: &[String]) -> Result<Vec<(String, Vec<String>)>>`: agrupa por nome-base (case-insensitive na extensão), aceita só `.pak/.utoc/.ucas` e ignora o resto. Se um grupo não tem `.pak`, retorna `Err("'<stem>' não tem arquivo .pak")`.
  - `fn mod_id(pak: &Path) -> Result<String>`: SHA-256 em streaming, 12 hex.

Comportamento de `import`:
- Se `source` é `.zip`: extrai para uma pasta temporária dentro de `dir`, ignorando entradas sem `enclosed_name()` e achatando subpastas. Se o zip não tem nenhum `.pak`: `Err("nenhum .pak encontrado no zip")`.
- Se `source` é `.pak/.utoc/.ucas`: usa os irmãos com o mesmo nome-base na mesma pasta.
- Qualquer outra extensão: `Err("formato não suportado: <ext>")`.
- Cada grupo é copiado para `mod_dir(id)`. Se o `id` já está no índice, não duplica: retorna a entrada existente.

- [ ] **Step 1: Testes.** Crie um helper `fn write(dir, name, bytes)` e um helper que monta um zip com `zip::ZipWriter`.

```rust
#[test] fn import_single_pak_copies_into_library()      // 1 entrada; files == ["A.pak"]; mod_dir(id)/A.pak existe; id.len()==12
#[test] fn import_pak_picks_up_utoc_ucas_siblings()     // A_P.pak/.utoc/.ucas + B.pak na pasta → importar A_P.pak dá files de A_P (3), name=="A"; B não entra
#[test] fn import_zip_with_nested_folders_finds_all_paks() // zip com "x/y/A.pak" e "B.pak" → 2 entradas
#[test] fn import_zip_without_pak_errors()               // zip só com readme.txt → Err contendo "nenhum .pak"
#[test] fn utoc_without_pak_errors()                     // group_pak_files(["A.utoc"]) → Err contendo "não tem arquivo .pak"
#[test] fn reimport_same_pak_does_not_duplicate()        // importa 2x → list().len()==1
#[test] fn remove_deletes_dir_and_index_entry()          // após remove: list() vazio e mod_dir não existe
#[test] fn zip_entry_with_parent_traversal_is_ignored()  // zip com "../evil.pak" e "ok.pak" → 1 entrada (ok) e nada fora de dir
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml library` → FAIL.
- [ ] **Step 3:** Implementar conforme Interfaces.
- [ ] **Step 4:** Mesmo comando → 8 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: mod library import (pak, utoc/ucas, zip)"`

---

### Task 4: Perfis

**Files:**
- Create: `src-tauri/src/profiles.rs`
- Modify: `src-tauri/src/lib.rs` (`mod profiles;`)

**Interfaces:**
- Consumes: `DataDir`, `load_json`, `save_json` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct Profile { pub id: String, pub name: String, pub mod_ids: Vec<String> }`
  - `pub struct Profiles { path: PathBuf }`
  - `impl Profiles { pub fn open(data: &DataDir) -> Self; pub fn list(&self) -> Result<Vec<Profile>>; pub fn create(&self, name: &str) -> Result<Profile>; pub fn rename(&self, id: &str, name: &str) -> Result<()>; pub fn delete(&self, id: &str) -> Result<()>; pub fn set_mods(&self, id: &str, mod_ids: Vec<String>) -> Result<()>; pub fn get(&self, id: &str) -> Result<Profile>; pub fn remove_mod_everywhere(&self, mod_id: &str) -> Result<()> }`
  - Se o arquivo não existe ou está vazio, `list()` cria e salva `Profile { id: "vanilla", name: "Vanilla", mod_ids: [] }`. O id dos perfis novos é `format!("p{}", unix_millis)`. Nome vazio ou só espaços: `Err("nome do perfil vazio")`. Id inexistente: `Err("perfil não encontrado")`.

- [ ] **Step 1: Testes**

```rust
#[test] fn empty_store_seeds_vanilla()           // list() == [Vanilla]
#[test] fn create_then_list()                    // create("Pinturas 2025") → list().len()==2, nome bate
#[test] fn set_mods_preserves_order()            // set_mods(id, ["b","a","c"]) → get(id).mod_ids == ["b","a","c"]
#[test] fn delete_removes_profile()
#[test] fn remove_mod_everywhere_strips_from_all_profiles()
#[test] fn blank_name_is_rejected()              // create("  ") is_err
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml profiles` → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** → 6 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: profiles store"`

---

### Task 5: Detecção do jogo

**Files:**
- Create: `src-tauri/src/game.rs`
- Modify: `src-tauri/src/lib.rs` (`mod game;`)

**Interfaces:**
- Consumes: `Result`, `msg` (Task 2).
- Produces:
  - `#[derive(Serialize, Clone, Debug, PartialEq)] pub struct GameInfo { pub name: String, pub root: PathBuf, pub mods_dir: PathBuf, pub app_id: Option<u32> }`
  - `pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf>`: pega o valor de cada linha `"path"  "<valor>"` e troca `\\` por `\`.
  - `pub fn parse_app_manifest(acf: &str) -> Option<(u32, String)>`: lê `"appid"` e `"installdir"`.
  - `pub fn find_paks_dir(root: &Path) -> Option<PathBuf>`: primeira subpasta imediata `X` de `root` em que `X/Content/Paks` existe (não considera a pasta `Engine`). Retorna `X/Content/Paks`.
  - `pub fn inspect_game(root: &Path) -> Result<GameInfo>`: `mods_dir = paks/"~mods"` (não cria a pasta). O `app_id` vem do `appmanifest_*.acf` em `root/../..` cujo `installdir` é igual ao nome da pasta `root` (sem diferenciar maiúsculas). Sem `Content/Paks`: `Err("pasta não parece um jogo Unreal: Content/Paks não encontrado")`.
  - `pub fn detect_games(steam_root: &Path) -> Vec<GameInfo>`: lê `steam_root/steamapps/libraryfolders.vdf` e, para cada biblioteca, as pastas em `steamapps/common` cujo nome começa com `MotoGP` (sem diferenciar maiúsculas) e que passam no `inspect_game`.
  - `#[cfg(windows)] pub fn steam_root_from_registry() -> Option<PathBuf>`: `HKCU\Software\Valve\Steam`, valor `SteamPath`.

- [ ] **Step 1: Testes** (as fixtures montam uma árvore Steam falsa em tempdir)

```rust
#[test] fn parse_library_folders_unescapes_backslashes() {
    let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}";
    assert_eq!(parse_library_folders(vdf), vec![PathBuf::from(r"C:\Program Files (x86)\Steam"), PathBuf::from(r"D:\SteamLibrary")]);
}
#[test] fn parse_app_manifest_reads_id_and_installdir() // "\"appid\"\t\t\"123\"" + "\"installdir\"\t\t\"MotoGP25\"" → Some((123,"MotoGP25"))
#[test] fn inspect_game_finds_mods_dir_and_app_id()     // common/MotoGP25/MotoGP25/Content/Paks + steamapps/appmanifest_123.acf → mods_dir termina em "Content\\Paks\\~mods", app_id Some(123)
#[test] fn inspect_game_without_paks_errors()
#[test] fn detect_games_only_returns_motogp_folders()   // common/MotoGP25 (válido) + common/OtherGame (válido) → só MotoGP25
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml game` → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** → 5 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: steam game detection"`

---

### Task 6: Aplicar e limpar mods

**Files:**
- Create: `src-tauri/src/deploy.rs`
- Modify: `src-tauri/src/lib.rs` (`mod deploy;`)

**Interfaces:**
- Consumes: `Library::mod_dir`, `ModEntry` (Task 3); `load_json`, `save_json` (Task 2).
- Produces:
  - `#[derive(Serialize, Deserialize, Default, Debug, PartialEq)] pub struct Deployed { pub mods_dir: Option<PathBuf>, pub files: Vec<String> }`
  - `pub fn deployed_name(index: usize, mod_id: &str, original: &str) -> String`
  - `pub fn deploy(lib: &Library, mods: &[ModEntry], mods_dir: &Path, manifest: &Path) -> Result<Deployed>`: primeiro chama `clean(manifest)`. Com `mods` vazio, retorna sem criar a pasta. Senão faz `create_dir_all(mods_dir)` e, para cada arquivo (o `index` é a posição do mod em `mods`), tenta `fs::hard_link` e cai para `fs::copy`. Se o destino já existe: `Err("arquivo já existe em ~mods: <nome>")`. O manifest é salvo **depois de cada arquivo**, para que um crash no meio ainda possa ser limpo.
  - `pub fn clean(manifest: &Path) -> Result<usize>`: apaga cada `mods_dir/file` listado (ignora `NotFound`), salva `Deployed::default()` e retorna quantos apagou.

- [ ] **Step 1: Testes**

```rust
#[test] fn deploy_writes_files_in_profile_order_with_prefix() // mods [B, A] → "000_<idB>_B.pak", "001_<idA>_A.pak"
#[test] fn deploy_preserves_p_suffix()                        // original "Ducati_P.pak" → nome termina com "_Ducati_P.pak"
#[test] fn clean_removes_only_deployed_files()                // "meu_mod.pak" manual em ~mods continua lá após clean
#[test] fn clean_tolerates_already_missing_file()             // apaga um à mão antes → clean Ok
#[test] fn deploy_cleans_previous_session_first()             // deploy [A]; deploy [B] → só arquivos de B em ~mods
#[test] fn deploy_refuses_to_overwrite_foreign_file()         // cria o nome-alvo à mão → Err contendo "já existe"; arquivo manual intacto
#[test] fn deploy_empty_profile_leaves_dir_untouched()        // mods [] → ~mods não é criado
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml deploy` → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** → 7 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: deploy and clean mods with manifest"`

---

### Task 7: Abrir o jogo e acompanhar a sessão

**Files:**
- Create: `src-tauri/src/launch.rs`
- Modify: `src-tauri/src/lib.rs` (`mod launch;`)

**Interfaces:**
- Consumes: `GameInfo` (Task 5).
- Produces:
  - `pub fn launch_url(game: &GameInfo) -> Result<String>` → `"steam://rungameid/{app_id}"`; sem `app_id`: `Err("App ID do jogo não encontrado; abra o jogo pela Steam uma vez")`.
  - `#[derive(Debug, PartialEq, Clone, Copy)] pub enum WatchEvent { Started, Ended, NeverStarted }`
  - `#[derive(Default)] pub struct WatchState { seen: bool, missing_polls: u32 }`
  - `pub fn watch_step(s: &mut WatchState, running: bool, elapsed: Duration) -> Option<WatchEvent>`: máquina de estados pura. Primeira vez `running` → `Started`. Depois de visto, 2 polls seguidos sem rodar → `Ended`; um poll rodando zera o contador. Nunca visto e `elapsed >= 120s` → `NeverStarted`.
  - `pub fn is_game_running(sys: &mut sysinfo::System, root: &Path) -> bool`: atualiza os processos e verifica se algum `exe()` começa com `root` (comparação sem diferenciar maiúsculas, já que é Windows).
  - `pub const POLL: Duration = Duration::from_secs(2);`

- [ ] **Step 1: Testes**

```rust
#[test] fn launch_url_uses_app_id()                 // app_id Some(123) → "steam://rungameid/123"
#[test] fn launch_url_without_app_id_errors()
#[test] fn started_then_ended_after_two_missing_polls() // true→Started; false→None; false→Ended
#[test] fn flapping_resets_missing_counter()        // true, false, true, false → nenhum Ended
#[test] fn never_started_after_120s()               // false @119s → None; false @120s → NeverStarted
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml launch` → FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** → 5 passed.
- [ ] **Step 5:** `git add -A; git commit -m "feat: launch url and session watcher"`

---

### Task 8: Comandos Tauri e recuperação ao iniciar

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (registrar plugins, estado, comandos e `setup`)

**Interfaces:**
- Consumes: tudo das Tasks 2–7.
- Produces (nomes que o frontend chama via `invoke`, com argumentos em camelCase do lado JS):
  - `pub struct AppState { pub data: DataDir, pub session_active: Arc<AtomicBool> }`
  - `get_config() -> Config`
  - `detect_games() -> Vec<GameInfo>`: no Windows, `steam_root_from_registry()` + `detect_games`; vazio se não achar a Steam.
  - `set_game_dir(path: String) -> GameInfo`: `inspect_game`, depois grava `config.game_dir`.
  - `get_game() -> Option<GameInfo>`: `inspect_game(config.game_dir)`, `None` se não houver.
  - `list_mods() -> Vec<ModEntry>`; `import_mods(paths: Vec<String>) -> Vec<ModEntry>` (concatena os resultados; na primeira falha retorna o erro); `remove_mod(id: String)` (também chama `remove_mod_everywhere`).
  - `list_profiles() -> Vec<Profile>`; `create_profile(name) -> Profile`; `rename_profile(id, name)`; `delete_profile(id)`; `set_profile_mods(id, modIds: Vec<String>)`; `set_active_profile(id)`.
  - `play(app: AppHandle, profileId: String) -> ()`: se `session_active` já é true, `Err("o jogo já está rodando")`. Senão: `get_game` (ou `Err("escolha a pasta do jogo")`), resolve os `ModEntry` do perfil na ordem, ignorando ids que não estão mais na biblioteca, chama `deploy` e abre `launch_url` com `tauri_plugin_opener::OpenerExt::opener().open_url`. Depois cria uma thread que faz poll com `POLL` / `watch_step` / `is_game_running`. Em `Ended` ou `NeverStarted`, chama `clean`, zera `session_active` e emite `app.emit("session", SessionEvent)`. Em `Started`, só emite.
  - `#[derive(Serialize, Clone)] pub struct SessionEvent { pub state: &'static str /* "started" | "ended" | "never-started" */, pub cleaned: usize }`
- `setup()` em `lib.rs`: `data = DataDir(app.path().app_data_dir()?)`. Chama `clean(data.deployed())`; se retornar > 0, faz `eprintln!` com o número (é a recuperação de crash). Depois registra o `AppState`.

- [ ] **Step 1: Teste da regra pura extraída** em `commands.rs`: `pub fn resolve_profile_mods(profile: &Profile, library: &[ModEntry]) -> Vec<ModEntry>`.

```rust
#[test] fn resolve_profile_mods_keeps_order_and_skips_missing() // mod_ids ["b","gone","a"] → [b, a]
```

- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml commands` → FAIL.
- [ ] **Step 3:** Implementar `commands.rs` e a fiação em `lib.rs` (`.plugin(tauri_plugin_dialog::init())`, `.plugin(tauri_plugin_opener::init())`, `.invoke_handler(tauri::generate_handler![...])`). Confira se `capabilities/default.json` tem `dialog:default` e `opener:default`; para abrir `steam://`, adicione a permissão `opener:allow-open-url` com `{ "url": "steam://*" }`.
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml` → todos passam (≥ 35). `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` → sem avisos.
- [ ] **Step 5:** `git add -A; git commit -m "feat: tauri commands, session thread, crash recovery"`

---

### Task 9: Interface

**Files:**
- Create: `src/api.ts`, `src/pages/Library.tsx`, `src/pages/Profiles.tsx`, `src/pages/Settings.tsx`
- Modify: `src/App.tsx`, `src/App.css` (remover o conteúdo do template)

**Interfaces:**
- Consumes: comandos e evento `"session"` da Task 8.
- Produces: em `src/api.ts`, os tipos `Config`, `GameInfo`, `ModEntry`, `Profile` e `SessionEvent`, espelhando o Rust em snake_case (`game_dir`, `mods_dir`, `app_id`, `mod_ids`, `size_bytes`, `active_profile`), e uma função tipada por comando (`listMods()`, `importMods(paths)`, `play(profileId)` etc.) usando `invoke` de `@tauri-apps/api/core`.

Telas:
- **Layout:** barra lateral (Biblioteca, Perfis, Configurações). Embaixo dela, o seletor do perfil ativo e o botão **Jogar**, desabilitado sem jogo configurado ou com sessão ativa. O estado da sessão vem de `listen("session", …)`: "Rodando…" / "Sessão encerrada — N arquivos removidos" / "O jogo não abriu em 2 min; mods removidos".
- **Biblioteca:** lista nome, tamanho e nº de arquivos, com botão remover. O botão "Importar" abre `open({ multiple: true, filters: [{ name: "Mods", extensions: ["pak","utoc","ucas","zip"] }] })` do `@tauri-apps/plugin-dialog`. Arrastar e soltar usa `getCurrentWebview().onDragDropEvent` (evento `drop` → `importMods(paths)`).
- **Perfis:** lista de perfis (criar/renomear/excluir; o "vanilla" não pode ser excluído). O perfil selecionado mostra checkboxes dos mods e botões ↑/↓ para a ordem, e salva com `setProfileMods`.
- **Configurações:** mostra o jogo atual (nome, `mods_dir`, `app_id`), a lista de `detectGames()` com botão "Usar este" e "Escolher pasta…" (`open({ directory: true })` → `setGameDir`).
- Todo `invoke` rejeitado mostra a mensagem do erro num aviso no topo.

- [ ] **Step 1:** `pnpm tsc --noEmit` → sem erros.
- [ ] **Step 2: Verificação manual** com `pnpm tauri dev`, sem o jogo, usando uma pasta falsa `C:\temp\MotoGPFake\MotoGPFake\Content\Paks`:
  - Configurações → escolher `C:\temp\MotoGPFake` → mostra o `mods_dir`, e o `app_id` vazio.
  - Importar um `.pak` qualquer (crie um com `fsutil file createnew C:\temp\teste.pak 1024`) → aparece na Biblioteca; importar de novo não duplica.
  - Criar o perfil "Teste" e marcar o mod.
  - Jogar → aviso "App ID do jogo não encontrado…" e nada criado em `~mods`.
- [ ] **Step 3:** `git add -A; git commit -m "feat: launcher UI"`

---

### Task 10: Build e teste real

- [ ] **Step 1:** `pnpm tauri build` → gera `src-tauri\target\release\motogp-launcher.exe` e o instalador em `src-tauri\target\release\bundle\`.
- [ ] **Step 2:** `(Get-Item src-tauri\target\release\motogp-launcher.exe).Length / 1MB` → < 15.
- [ ] **Step 3: Teste com o jogo real**
  - Configurações → Detectar → aparece o MotoGP instalado, com `app_id` preenchido.
  - Importar um mod de verdade (ex.: pintura) → perfil "Teste" → Jogar → o mod aparece no jogo.
  - Durante o jogo, `~mods` tem os arquivos `000_…`. Feche o jogo → em ~4 s o aviso "Sessão encerrada" aparece e `~mods` volta ao que era antes.
  - Coloque um `.pak` seu em `~mods` à mão, faça uma sessão e confirme que ele continua lá.
  - Crash: Jogar, mate o launcher pelo Gerenciador de Tarefas, feche o jogo e abra o launcher → `~mods` está limpo.
- [ ] **Step 4:** `git add -A; git commit -m "chore: release build verified"`
