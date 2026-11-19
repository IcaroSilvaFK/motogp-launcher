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
| 7 Coverage | `cargo llvm-cov --workspace --json --summary-only --output-path coverage-rust.json --ignore-filename-regex 'src-tauri[\\/]src[\\/](main\|bootstrap)\.rs'` then `node scripts/check-rust-coverage.mjs coverage-rust.json` | `pnpm test --coverage` |

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
| ZIP bomb protection (per-entry, total bytes, entry count, enforced while streaming) | `default_limits_are_generous_but_finite`, `entry_over_the_per_entry_cap_is_rejected_while_streaming`, `total_extracted_bytes_over_the_cap_are_rejected`, `archive_with_too_many_entries_is_rejected_before_extracting` (ZIP adapter), `zip_over_the_limits_is_rejected_and_the_staging_dir_is_cleaned_up` (infra) |
| Publish never reuses an existing ID folder | `publish_never_reuses_an_existing_unindexed_id_folder` (fs adapter) |
| A multi-group ZIP import is all-or-nothing | `zip_import_with_a_failing_second_group_leaves_the_index_unchanged`, `zip_import_index_failure_removes_every_folder_published_by_the_import` (application) |
| Removal deletes the folder first and is retryable | `failed_folder_deletion_keeps_the_entry_and_a_retry_succeeds`, `remove_retries_when_the_folder_is_gone_but_the_entry_remains` (application) |
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
| Ignore `//` comments in VDF/ACF | `parse_library_folders_skips_comments`, `parse_app_manifest_skips_comments` (infrastructure `steam.rs`) |
| Read non-overlapping ACF key/value pairs | `parse_app_manifest_ignores_overlapping_pairs` (infrastructure `steam.rs`) |
| Skip unreadable libraries during automatic detection | `detect_games_skips_unreadable_libraries_and_keeps_scanning` (application, fake port) |
| Registry precedence and HKLM fallback | `hkcu_wins_over_hklm`, `falls_back_to_hklm_when_hkcu_is_missing`, `falls_back_to_hklm_when_hkcu_is_blank`, `no_steam_anywhere_is_none` (infrastructure `windows_registry.rs`, injectable `RegistryReader`) |
| Inspection has no write effects | `inspection_does_not_create_mods_directory` (infrastructure) |
| Error mapping at the Tauri boundary | `src-tauri/src/error_game.rs` unit tests (`not_unreal_game` code) |

### Issue #7: scenario traceability

Gherkin: `crates/domain/tests/features/session-state.feature` (steps `tests/bdd/session_state.rs`), `crates/application/tests/features/session-tracking.feature` (fake clock/inspector, `tests/bdd/session_tracking.rs`) and `crates/infrastructure/tests/features/processes.feature` (controlled paths, `tests/bdd/processes.rs`). No test starts a real game or sleeps for real.

| Scenario | Test |
|----------|------|
| Build the URL with a detected App ID | `launch_url_uses_app_id` (domain), `launch_opens_the_steam_url` (application) + `session-state.feature` |
| Refuse to launch without an App ID | `launch_url_without_app_id_errors`, `launch_without_app_id_does_not_open_anything` + `session-state.feature` |
| End after two consecutive absences | `started_then_ended_after_two_missing_polls`, `terminal_watch_event_is_not_repeated` + `session-state.feature` |
| Reset absences when the process reappears | `flapping_resets_missing_counter` + `session-state.feature` |
| Report a game that never started | `never_started_after_120s`, `watcher_reports_never_started_at_120_seconds_without_sleeping_for_real` + `session-state.feature` |
| Observe only executables inside the root (junctions, `..`, verbatim prefix; lexical fallback; 8.3 short names rely on canonicalize and are not tested) | `process_path_comparison_is_case_insensitive`, `sibling_directory_is_not_game_root`, `missing_process_executable_is_ignored`, `dot_dot_segments_are_resolved_before_comparing`, `verbatim_prefix_matches_the_plain_path`, `executable_under_a_junction_to_the_root_is_inside_it`, `real_inspector_reuses_its_state_and_finds_the_current_process` (infrastructure `processes.rs`) + `processes.feature` |
| Use the defined tracking interval | `watcher_polls_every_two_seconds` (application) + `session-tracking.feature` |
| Poll on a fixed schedule, no drift from inspection time | `inspection_time_does_not_drift_the_poll_schedule`, `never_started_still_triggers_at_120_seconds_with_slow_inspections`, `overrunning_poll_runs_the_next_one_immediately_without_drift` (application) |
| Cancel tracking without a fake game event | `cancelled_before_start_never_inspects_nor_emits`, `cancelled_while_running_stops_without_a_fake_ended` (`TrackOutcome::Cancelled`) |

Deviation: the `UrlOpener`, `ProcessInspector` and `Clock` ports are defined in `application/src/session_ports.rs`. The real opener (Tauri opener plugin) is wired in issue #8.

### Manual smoke: Steam registry (Windows)

The precedence/fallback logic is unit-tested through the injectable `RegistryReader`; only the real `winreg` reader (`read_steam_path`, `SteamLibraries::from_registry`) is verified by this smoke. With Steam installed, `HKCU\Software\Valve\Steam` must have a `SteamPath` value (otherwise `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` `InstallPath` is used); call `SteamLibraries::from_registry().library_folders()` (or the future detect command) and confirm it lists the Steam install and every library in `libraryfolders.vdf`. With the value missing, the result must be an empty list.

### Manual smoke: `pnpm tauri dev`

Prerequisites: MSVC build tools, Rust stable (MSVC), Node 22, pnpm (corepack), WebView2 Runtime. Run `pnpm tauri dev` and confirm the "MotoGP Launcher" window opens with a 1100x700 client area (minimum 900x600) and shows the "MotoGP Launcher" heading. Verified on Windows 11 during issue #1.

### BDD runners per crate

`domain`, `application` and `infrastructure` each have one `features` test target (`tests/features.rs`, `harness = false`). It is a dispatcher: every feature owns `tests/bdd/<feature>.rs` with its own `World` and `pub async fn run() -> bool` that runs only `tests/features/<feature>.feature`. Adding a feature means one `#[path]` module line and one `FEATURES` entry, so parallel issues only touch those lines.

### Scenario traceability (issue #3)

Gherkin: `crates/domain/tests/features/mod-grouping.feature` (pure rules), `crates/application/tests/features/library.feature` (in-memory fakes, `crates/application/tests/support/fakes.rs`) and `crates/infrastructure/tests/features/file-import.feature` (real fs/ZIP/SHA-256 in temp dirs). Step modules: `tests/bdd/mod_grouping.rs`, `tests/bdd/library.rs`, `tests/bdd/file_import.rs`. Use-case tests: `crates/application/tests/mod_service.rs`; adapter integration tests: `crates/infrastructure/tests/library.rs`.

| Scenario | Test |
|----------|------|
| Import a loose pak | `import_single_pak_copies_into_library` (infra), `import_single_pak_registers_the_entry` (application) + cucumber in `library.feature` and `file-import.feature` |
| Pick up only the siblings of the chosen mod | `import_pak_picks_up_utoc_ucas_siblings` (infra), `import_picks_only_the_siblings_of_the_chosen_mod` (application) + cucumber |
| Find mods in the subfolders of a ZIP | `import_zip_with_nested_folders_finds_all_paks` (infra), `import_zip_returns_every_valid_group` (application) + cucumber |
| Reject a ZIP without a pak | `import_zip_without_pak_errors` (infra), `zip_without_pak_errors` (application) + cucumber |
| Reject a group without a pak | `utoc_without_pak_errors` (domain), `import_utoc_without_pak_sibling_errors` (application) + cucumber in `mod-grouping.feature` |
| Reimport without duplicating | `reimport_same_pak_does_not_duplicate` (application), `reimport_same_content_returns_the_existing_entry` (infra) + cucumber |
| Remove a mod | `remove_deletes_dir_and_index_entry` (infra), `remove_deletes_the_folder_and_the_index_entry` (application) + cucumber |
| Ignore directory traversal in a ZIP | `zip_entry_with_parent_traversal_is_ignored` (infra), `entries_without_a_safe_name_are_ignored` (ZIP adapter) + cucumber |
| Reject a collision when flattening subfolders | `flattened_zip_name_collision_is_rejected` (infra), `colliding_flattened_names_are_rejected_without_overwriting` (ZIP adapter) + cucumber |
| Reject an unsupported format | `unsupported_extension_is_rejected` (application), `unsupported_format_is_rejected_before_touching_the_library` (infra) + cucumber |
| A failed import publishes nothing | `failed_import_does_not_publish_index_entry`, `index_failure_rolls_back_the_published_folder` (application) + cucumber |
| Mod ID is the first 12 SHA-256 hex characters | `mod_id_matches_first_twelve_sha256_hex` (infra, SHA-256 of `abc`), `mod_id_is_the_first_twelve_lowercase_hex_characters` (domain) |
| Case-insensitive extensions, unrelated extensions ignored, display name | `group_accepts_case_insensitive_extensions`, `group_ignores_unrelated_extensions`, `display_name_strips_only_the_final_p_suffix` (domain) + cucumber |
| Remove only validated IDs | `remove_rejects_ids_that_are_not_twelve_lowercase_hex` (application), `remove_rejects_ids_that_could_escape_the_library` (infra), `remove_mod_dir_deletes_only_the_validated_id_folder` (fs adapter) |
| Error mapping at the Tauri boundary | `src-tauri/src/error_library.rs` unit tests (`From<LibraryError> for CommandError`) |

### Scenario traceability (issue #6)

Gherkin: `crates/domain/tests/features/deploy-names.feature` (pure naming and name safety), `crates/application/tests/features/deploy.feature` (in-memory fakes with injected failures and a simulated process death, `crates/application/tests/support/deploy_fakes.rs`) and `crates/infrastructure/tests/features/file-deploy.feature` (real fs and `deployed.json` in temp dirs). Step modules: `tests/bdd/deploy_names.rs`, `tests/bdd/deploy.rs`, `tests/bdd/file_deploy.rs`. Use-case tests: `crates/application/tests/deploy_service.rs`; adapter contracts: `crates/infrastructure/tests/deploy.rs`.

Recovery protocol (write-ahead journal in `deployed.json`): the destination is verified absent, journaled in `pending`, created exclusively (hard link, copy fallback with `create_new`), then moved to `files`. Cleanup removes `files` + `pending` only (never by name or prefix scan), tolerating `NotFound`, and retains what it could not remove. A `pending` entry is owned because its name was absent when it was journaled and creation is exclusive; if the exclusive create loses a race to a foreign file the entry is untracked again.

| Scenario | Test |
|----------|------|
| Deploy in the profile order | `deploy_writes_files_in_profile_order_with_prefix` (application), `deploy_writes_files_in_profile_order_with_prefix_on_disk` (infra), `every_deployed_file_has_a_persisted_record_before_it_exists` + cucumber in all three features |
| Preserve the patch suffix | `deploy_preserves_p_suffix` (domain) + cucumber in `deploy-names.feature` |
| Clean only what was deployed | `clean_removes_only_deployed_files` (infra), `clean_removes_only_registered_files_and_reports_the_count` (application) + cucumber |
| Tolerate an already-missing deployed file | `clean_tolerates_already_missing_file` (infra), `clean_tolerates_an_already_missing_file_and_counts_only_removed_ones` (application) + cucumber |
| Clean the previous session before redeploying | `deploy_cleans_previous_session_first`, `deploy_does_not_proceed_when_cleaning_the_previous_session_fails` (application) + cucumber |
| Refuse a collision with a user file | `deploy_refuses_to_overwrite_foreign_file`, `copy_fallback_never_overwrites_existing_file`, `adapter_refuses_to_overwrite_an_existing_destination_by_hard_link` (infra), `deploy_refuses_a_destination_that_already_exists_and_leaves_it_alone` (application) + cucumber |
| Deploy an empty profile without creating a directory | `deploy_empty_profile_leaves_dir_untouched` (application), `deploy_empty_profile_does_not_create_the_directory` (infra) + cucumber |
| Recover from an interruption during deploy | `interrupted_deploy_remains_recoverable` (application, four crash points) + cucumber (application and infra, journal left on disk) |
| Retry a partially failed cleanup | `failed_clean_keeps_pending_entries` (application), `failed_clean_on_disk_keeps_pending_entries_for_a_retry` (infra) + cucumber |
| Reject a path outside the authorized directory | `invalid_manifest_path_is_rejected` (application), `invalid_manifest_path_on_disk_is_rejected`, `adapter_rejects_names_that_could_escape_the_directory` (infra) + cucumber |
| Hard link, with copy fallback | `hard_link_is_used_when_it_succeeds`, `hard_link_failure_falls_back_to_copy`, `a_failed_copy_leaves_no_partial_destination` (infra, controlled link failure) |
