# Validation workflow: TDD + BDD + Clean Architecture

Mandatory for **every task**. No PR is opened without going through all the steps.

Stack: Tauri 2 + Rust (core), React + TypeScript + Vite (frontend), pnpm.

---

## 1. BDD: specify the behavior (Codex, in the issue)

Every issue carries the acceptance criteria in **Gherkin**:

```gherkin
Feature: Install mod
  Scenario: A valid mod is added to the library
    Given the user has an active profile
    When they import the file "mod.zip"
    Then the mod appears in the library
    And the library.json file is updated
```

- Scenarios become `.feature` files in the repo:
  - Rust: `tests/features/*.feature`, run with the [`cucumber`](https://crates.io/crates/cucumber) crate.
  - Frontend: scenarios become `describe("Given…") / it("When… Then…")` in Vitest + Testing Library.
- If a scenario cannot be tested, the issue is poorly defined. It goes back to Codex.

## 2. TDD: Red → Green → Refactor (Dev, Sonnet)

For each scenario/rule:

1. **Red:** write the test first and run it to see it **fail** for the right reason.
2. **Green:** write the minimum code to make the test pass.
3. **Refactor:** clean up with the tests green.

Rules:
- Commits show the order: the `test:` commit comes before or together with the `feat:` one. Production code without a test covering it is rejected.
- The domain and use cases are tested **without** Tauri, file system or network (only fakes of the ports).
- Minimum coverage: **domain/application ≥ 90%**, overall ≥ 80%.

## 3. Clean Architecture: structure and dependency rule

Dependencies always point **inward**: `interface → infrastructure → application → domain`.

### Rust (Cargo workspace, separation enforced by the compiler)

```
crates/
  domain/          # entities, value objects, rules. No external deps (no IO serde, no tauri)
  application/     # use cases + ports (traits): ModRepository, FileSystem, Downloader…
  infrastructure/  # adapters implementing the ports (fs, http, json)
src-tauri/         # interface layer: thin #[tauri::command]s that only call use cases
```

- `domain` does not depend on any other crate of the project.
- `application` depends only on `domain`.
- Tauri commands hold no business rules: they convert the DTO, call the use case and map the error.

### Frontend (React + TS)

```
src/
  domain/          # pure types and rules
  application/     # hooks/use cases that depend on interfaces (ports)
  infrastructure/  # adapters: Tauri invoke(), storage
  ui/              # React components (presentation only + calls to use cases)
```

- Boundaries validated with `eslint-plugin-boundaries` (or `dependency-cruiser`): `ui` does not import `infrastructure` directly, and `domain` imports nothing from outside.

## 4. Validation gates (QA Haiku → Codex)

**QA (Haiku)** runs and checks, in order:

| # | Gate | Command |
|---|------|---------|
| 1 | Formatting | `cargo fmt --check` · `pnpm prettier --check .` |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` · `pnpm lint` |
| 3 | Types | `pnpm tsc --noEmit` |
| 4 | Unit tests (TDD) | `cargo test --workspace` · `pnpm test` |
| 5 | BDD scenarios | `cargo test --test features` · Vitest scenarios |
| 6 | Architecture | crate deps checked + `pnpm lint` (boundaries) |
| 7 | Coverage | `cargo llvm-cov` · `pnpm test --coverage` (targets above) |
| 8 | Acceptance criteria | every Gherkin scenario in the issue has a matching, passing test |

If any gate fails, the delivery goes back to the dev with the QA report.

**Codex** then reviews:
- whether the TDD order is visible in the commits;
- whether the dependency rule and the per-layer responsibilities were respected;
- whether the tests are good: behavior instead of implementation, no excessive mocks;
- suggested improvements and refactors, which are applied before the PR.

## 5. PR checklist

- [ ] Issue with Gherkin scenarios linked (`Closes #<no>`)
- [ ] Tests written before the code (Red → Green → Refactor)
- [ ] BDD scenarios implemented and passing
- [ ] Domain/application with no dependency on infra/Tauri
- [ ] All gates green (local and CI)
- [ ] QA approved + Codex review applied

The CI ([.github/workflows/validacao.yml](../.github/workflows/validacao.yml)) runs the same gates on every PR.

---

## 6. Scaffold (issue #1): how the gates run

Run everything from the workspace root. `pnpm` is declared through `packageManager` in `package.json` (`corepack pnpm ...` works without a global install).

| Gate | Rust | Frontend |
|------|------|----------|
| 1 Formatting | `cargo fmt --all --check` | `pnpm prettier --check .` (docs/ and `*.md` are ignored on purpose) |
| 2 Lint | `cargo clippy --workspace --all-targets -- -D warnings` | `pnpm lint` (ESLint + `eslint-plugin-boundaries`) |
| 3 Types | built into `cargo` | `pnpm tsc --noEmit` |
| 4 Unit tests | `cargo test --workspace` | `pnpm test` (`vitest run`, non-interactive) |
| 5 BDD | `cargo test --workspace --test features` (or `-p motogp-launcher --test features`) | Vitest scenarios (`src/ui/__tests__`, `tooling/__tests__`, Gherkin in `tests/features/frontend/`) |
| 6 Architecture | `cargo test -p motogp-launcher --test architecture` (reads `cargo metadata`) | `pnpm lint` (boundaries) |
| 7 Coverage | `cargo llvm-cov --workspace --json --summary-only --output-path coverage-rust.json --ignore-filename-regex '(^\|[\\/])\.worktrees[\\/]\|src-tauri[\\/]src[\\/](main\|bootstrap)\.rs'` then `node scripts/check-rust-coverage.mjs coverage-rust.json` | `pnpm test --coverage` |

Limits live in `tooling/coverage-thresholds.json` (`core` = 90, `overall` = 80) and are shared by Vitest, the Rust coverage script and the BDD checks.

### Initial coverage condition (no core lines yet)

`motogp-domain`, `motogp-application`, `src/domain` and `src/application` have no executable lines in issue #1. No fictitious business rule was added and no limit was relaxed:

- Rust: `scripts/check-rust-coverage.mjs` prints an explicit `NOTE ... no measurable lines yet` for an empty core crate and enforces 90% per crate as soon as the report contains lines for it.
- Frontend: Vitest thresholds (`src/domain/**` and `src/application/**` at 90%, 80% overall) apply to any code that lands there.
- The only coverage exclusions are narrow: the Tauri bootstrap (`src-tauri/src/main.rs`, `bootstrap.rs`, only runs with a real window), `src/main.tsx` (composition root that mounts React) and test/declaration files.
- `coverage_thresholds_reject_insufficient_coverage` proves that insufficient reports fail the gate.

### Scenario traceability (issue #1)

| Scenario | Test |
|----------|------|
| Generate the scaffold preserving the project | `src-tauri/tests/features/scaffold.feature` (cucumber) + `workspace_members_are_registered` |
| Open the application on Windows | cucumber scenario `Validate the Windows application configuration` checks `tauri.conf.json` (title, 1100x700, min 900x600, identifier); `renders_launcher_shell` checks rendering; the actual window opening is the manual smoke below |
| Reject dependencies that violate the architecture | `rejects_ui_importing_infrastructure` (Vitest, real ESLint on `tests/fixtures/boundaries`), `rejects_outward_cargo_dependencies` (`src-tauri/tests/architecture.rs`, fixtures in `src-tauri/tests/fixtures`) |
| Run all gates in CI | cucumber step + `tooling/__tests__/ci-workflow.test.ts` inspect `.github/workflows/validacao.yml`; the CI itself runs the gates |

### Scenario traceability (issue #2)

Gherkin: `crates/infrastructure/tests/features/persistence.feature`, run by `crates/infrastructure/tests/features.rs` (cucumber, temp dirs only). Application scenarios use a fake `ConfigRepository` (no fs).

| Scenario | Test |
|----------|------|
| Read a missing configuration | `missing_file_returns_default` + cucumber scenario |
| Save and read in a path with spaces and accents | `roundtrip_in_path_with_accents` + cucumber scenario |
| Refuse corrupted JSON | `corrupted_json_is_an_error`, `zero_byte_file_is_an_error_not_a_default` + cucumber scenario |
| Replace existing data safely on Windows | `save_replaces_existing_file_atomically` + cucumber scenario |
| Preserve the previous version when the write fails | `failed_save_preserves_previous_file` (a directory on `<path>.tmp` forces the failure), `failed_replace_removes_the_temp_file` + cucumber scenario |
| Application error propagation | `config_repository_error_is_propagated` (application, fake port) |
| Domain defaults | `default_config_has_no_game_dir_and_no_active_profile` (domain) |
| Error mapping at the Tauri boundary | `src-tauri/src/error.rs` unit tests (`CommandError { code, message }`) |

### Issue #4: scenario traceability

Gherkin: `crates/application/tests/features/profiles.feature` (fake repository and fake clock, run by `crates/application/tests/bdd/profiles.rs`) and `crates/infrastructure/tests/features/profile-persistence.feature` (temp dirs, run by `crates/infrastructure/tests/bdd/profile_persistence.rs`).

| Scenario | Test |
|----------|------|
| Seed missing or empty profiles | `empty_store_seeds_vanilla` (application) + cucumber; infrastructure: `empty_collection_is_seeded_and_persisted` + "Seed a missing profiles file" / "Seed a valid empty collection" |
| Create and list a profile | `create_then_list` + cucumber; `profile_ids_do_not_collide`, `rapid_creations_with_the_real_clock_do_not_collide` |
| Preserve the mod order | `set_mods_preserves_order` + cucumber; infrastructure: `profiles_roundtrip_preserves_order` + "Roundtrip preserves the mod order" |
| Delete an ordinary profile | `delete_removes_profile` + cucumber |
| Remove references from all profiles | `remove_mod_everywhere_strips_from_all_profiles` (+ domain `remove_mod_strips_every_occurrence_and_keeps_order`) + cucumber |
| Reject a blank name | `blank_name_is_rejected` (+ domain `normalize_name_trims_and_rejects_blank`) + cucumber |
| Protect Vanilla in the backend | `vanilla_cannot_be_deleted` + cucumber |
| Rename a profile and handle an unknown ID | `rename_persists_new_name`, `unknown_profile_returns_error` + cucumber |
| Invalid storage is never reseeded | `invalid_storage_is_propagated_and_not_reseeded` (application), `invalid_profiles_json_is_not_reseeded` + cucumber ("Do not reseed invalid JSON", "Do not reseed a zero-byte file") |
| Error mapping at the Tauri boundary | `error_profiles::tests::maps_business_rules_to_stable_codes_and_messages`, `error_profiles::tests::storage_errors_reuse_the_app_error_mapping` (`src-tauri/src/error_profiles.rs`) |

### Issue #5: scenario traceability

Gherkin: `crates/application/tests/features/game-detection.feature` (fake `SteamLocator`/`GameFiles` ports, no disk; steps in `tests/bdd/game_detection.rs`) and `crates/infrastructure/tests/features/steam.feature` (temporary Steam tree; steps in `tests/bdd/steam.rs`). Both are registered in their crate's `tests/features.rs` dispatcher.

| Scenario | Test |
|----------|------|
| Parse Steam libraries with escaped backslashes | `parse_library_folders_unescapes_backslashes`, `parse_library_folders_supports_the_legacy_format` (infrastructure `steam.rs`) + `steam.feature` |
| Read the App ID and the install directory | `parse_app_manifest_reads_id_and_installdir`, `parse_app_manifest_rejects_incomplete_or_invalid_input` + `steam.feature` |
| Inspect a valid Unreal game | `inspect_game_finds_mods_dir_and_app_id` (infrastructure `game_files.rs`), `manifest_match_is_case_insensitive` (application) + both features |
| Reject a root without project Paks | `inspect_game_without_paks_errors`, `root_without_paks_is_not_an_unreal_game`, `error_display_says_content_paks_not_found` + both features |
| Detect only MotoGP | `detect_games_only_returns_motogp_folders`, `detect_games_returns_only_valid_motogp_folders`, `motogp_prefix_is_case_insensitive` (domain) + both features |
| Ignore the Engine folder | `engine_folder_is_not_selected` (application), `engine_folder_is_skipped_on_disk`, `engine_folder_ignores_case` (domain) + both features |
| Allow a manual folder without a Steam manifest | `manual_game_without_manifest_has_no_app_id` + both features |
| Steam not found in the registry | `missing_steam_returns_empty_list`, `steam_libraries_without_steam_path_is_empty` + both features |
| Inspection has no write effects | `inspection_does_not_create_mods_directory` (infrastructure) |
| Error mapping at the Tauri boundary | `src-tauri/src/error_game.rs` unit tests (`not_unreal_game` code) |

### Manual smoke: Steam registry (Windows)

The `winreg` adapter (`read_steam_path`, `SteamLibraries::from_registry`) is behind `#[cfg(windows)]` and is deliberately not covered by automated tests. Smoke: with Steam installed, `HKCU\Software\Valve\Steam` must have a `SteamPath` value; call `SteamLibraries::from_registry().library_folders()` (or the future detect command) and confirm it lists the Steam install and every library in `libraryfolders.vdf`. With the value missing, the result must be an empty list.

### Manual smoke: `pnpm tauri dev`

Prerequisites: MSVC build tools, Rust stable (MSVC), Node 22, pnpm (corepack), WebView2 Runtime. Run `pnpm tauri dev` and confirm the "MotoGP Launcher" window opens with a 1100x700 client area (minimum 900x600) and shows the "MotoGP Launcher" heading. Verified on Windows 11 during issue #1.

### BDD runners per crate

`domain`, `application` and `infrastructure` each have one `features` test target (`tests/features.rs`, `harness = false`). It is a dispatcher: every feature owns `tests/bdd/<feature>.rs` with its own `World` and `pub async fn run() -> bool` that runs only `tests/features/<feature>.feature`. Adding a feature means one `#[path]` module line and one `FEATURES` entry, so parallel issues only touch those lines.
