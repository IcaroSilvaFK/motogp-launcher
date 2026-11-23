# Validation workflow: TDD + BDD + Clean Architecture

Mandatory for **every task**. No PR is opened without going through all the steps.

Stack: Tauri 2 + Rust (core), React + TypeScript + Vite (frontend), pnpm.

---

## 1. BDD: specify the behavior (Coordinator, in the issue)

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
  - Frontend: `.feature` files live in `tests/features/frontend/` and are executed with [`@amiceli/vitest-cucumber`](https://github.com/amiceli/vitest-cucumber). Each file is bound by a step-definition spec (`src/ui/__tests__/<name>.feature.test.tsx`) that calls `loadFrontendFeature("<name>")` (`src/ui/__tests__/features.ts`, path resolved from the file location, so it works in worktrees and CI) and `describeFeature`, with one `Scenario` per Gherkin scenario and one step function per Given/When/Then/And (Testing Library for the UI). They run under `corepack pnpm test` (`vitest run`), with no extra CI step. The binding is strict: a scenario added or renamed in the `.feature` without matching step code, a step that is never called, or step code for a scenario that no longer exists makes the suite fail. Do not write a hand-written Vitest test that restates a bound scenario.
- If a scenario cannot be tested, the issue is poorly defined. It goes back to the Coordinator.

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

## 4. Validation gates (QA Haiku)

**QA (Haiku)** runs and checks, in order:

| # | Gate | Command |
|---|------|---------|
| 1 | Formatting | `cargo fmt --check` · `pnpm prettier --check .` |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` · `pnpm lint` |
| 3 | Types | `pnpm tsc --noEmit` |
| 4 | Unit tests (TDD) | `cargo test --workspace` · `pnpm test` |
| 5 | BDD scenarios | `cargo test --test features` · `vitest run` binds `tests/features/frontend/*.feature` through `@amiceli/vitest-cucumber` |
| 6 | Architecture | crate deps checked + `pnpm lint` (boundaries) |
| 7 | Coverage | `cargo llvm-cov` · `pnpm test --coverage` (targets above) |
| 8 | Acceptance criteria | every Gherkin scenario in the issue has a matching, passing test |

If any gate fails, QA sends its report to the Coordinator, who dispatches a new dev with the fix. When every gate passes, QA notifies the PO, who commits, opens the PR and closes the issue.

QA also checks:
- whether every rule and scenario has a test written for it (TDD), not code without tests;
- whether the dependency rule and the per-layer responsibilities were respected;
- whether the tests are good: behavior instead of implementation, no excessive mocks.

## 5. PR checklist

- [ ] Issue with Gherkin scenarios linked (`Closes #<no>`)
- [ ] Tests written before the code (Red → Green → Refactor)
- [ ] BDD scenarios implemented and passing
- [ ] Domain/application with no dependency on infra/Tauri
- [ ] All gates green (local and CI)
- [ ] QA approved
- [ ] Rebased onto the current `dev` and integration gate green (CLAUDE.md, git flow step 6)

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
| 5 BDD | `cargo test --workspace --test features` (or `-p motogp-launcher --test features`) | `corepack pnpm test`: Gherkin in `tests/features/frontend/` bound by `src/ui/__tests__/*.feature.test.tsx` (and `tooling/__tests__/scaffold.feature.test.tsx`, which runs Node tooling) with `@amiceli/vitest-cucumber` (unbound or removed scenarios fail), plus the other Vitest tests in `src` and `tooling/__tests__` |
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
| Open the application on Windows | cucumber scenario `Validate the Windows application configuration` checks `tauri.conf.json` (title, 1100x700, min 900x600, identifier); the frontend scenario `Render the launcher shell` (`tooling/__tests__/scaffold.feature.test.tsx`) checks rendering; the actual window opening is the manual smoke below |
| Reject dependencies that violate the architecture | scenario "Reject an import from ui to infrastructure" (bound in `tooling/__tests__/scaffold.feature.test.tsx`, real ESLint on `tests/fixtures/boundaries`), `rejects_outward_cargo_dependencies` (`src-tauri/tests/architecture.rs`, fixtures in `src-tauri/tests/fixtures`) |
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

Deviation: the `UrlOpener`, `ProcessInspector` and `Clock` ports are defined in `application/src/session_ports.rs`. The real opener (Tauri opener plugin, restricted to `steam://`) is wired in issue #8 (`src-tauri/src/adapters.rs`).

### Issue #8: scenario traceability

Gherkin: `crates/application/tests/features/commands-and-recovery.feature` (12 scenarios, steps `tests/bdd/commands_and_recovery.rs`, fake ports; the deploy side is the real `DeployService` over the in-memory deploy ports) and `src-tauri/tests/features/tauri-interface.feature` (5 scenarios, steps `tests/ipc_support/interface_steps.rs`, real commands over the Tauri mock runtime with a temp data dir and controlled opener, process, runner and event ports). No test starts a real game, opens Steam or sleeps.

| Scenario | Test |
|----------|------|
| Resolve mods in order ignoring missing references | `resolve_profile_mods_keeps_order_and_skips_missing` (`tests/commands.rs`) |
| Recover a previous deployment on startup | `startup_recovers_previous_deployment`, `startup_recovery_failure_is_visible_and_blocks_new_deploys`, `play_before_startup_recovery_is_rejected` (`tests/play_service.rs`); `setup_runs_recovery_before_accepting_play`, `a_recovery_failure_is_visible_and_blocks_play` (`src-tauri/tests/commands_contract.rs`) |
| Prevent a simultaneous session | `concurrent_play_is_rejected` (`tests/play_service.rs`), `concurrent_play_through_ipc_reports_the_running_game`; `only_one_thread_wins_a_race_for_the_lock` (infrastructure) |
| Validate prerequisites before deploy | `play_without_game_does_not_deploy`, `play_without_app_id_does_not_deploy` |
| Finish a session with cleanup and event | `ended_session_cleans_and_emits_count`, `never_started_session_cleans_and_releases_lock` |
| Emit start without cleaning prematurely | `started_event_does_not_clean` |
| Compensate a failure to open Steam | `opener_failure_rolls_back_deployment`, `opener_failure_keeps_manual_files_untouched`, `failed_compensation_is_reported_and_blocks_new_deploys` |
| Do not hide a recovery failure | `cleanup_failure_keeps_recovery_pending`, `tracking_failure_is_reported_without_a_false_success` |
| Keep profiles consistent when removing data | `remove_mod_clears_profile_references`, `deleting_active_profile_selects_vanilla`, `set_active_profile_validates_the_profile`, `remove_mod_reports_library_failures_and_keeps_references` |
| Batch import contract | `batch_import_stops_at_first_error_and_keeps_completed_imports`, `import_mods_reports_the_first_failure_and_keeps_completed_imports` (IPC) |
| Game configuration | `set_game_dir_inspects_before_saving`, `get_game_is_none_only_when_unconfigured`, `get_game_surfaces_inspection_errors_of_a_configured_path` |
| Never clean or redeploy while the game is still running | `restart_with_the_game_running_keeps_files_and_resumes_tracking`, `startup_inspection_failure_blocks_and_deletes_nothing`, `play_is_refused_while_the_game_is_running`, `play_inspection_failure_blocks_deployment`, `tracking_failure_with_the_game_running_never_redeploys_until_it_ends`, `tracking_failure_then_inspection_failure_deletes_nothing` (`tests/play_recovery.rs`) |
| Release the session lock when the worker fails | `panic_while_tracking_releases_the_lock_and_marks_recovery_pending`, `panic_while_preparing_releases_the_lock_and_marks_recovery_pending`, `spawn_failure_keeps_files_marks_recovery_pending_and_reports_an_error` (`tests/play_recovery.rs`); `runner_executes_the_task_on_another_thread` (infrastructure) |
| Keep the startup recovery result readable | `recovery_status_keeps_a_startup_failure_retrievable`, `recovery_status_reports_the_cleaned_count` (application); `startup_recovery_failure_stays_retrievable_through_recovery_status`, `successful_recovery_is_reported_with_its_cleaned_count` (IPC) |
| Keep the interface responsive and consistent | `concurrent_imports_do_not_lose_updates`, `new_play_errors_have_stable_codes` (`src-tauri/tests/commands_contract.rs`) |
| Allow a single launcher instance | `a_second_acquisition_fails_while_the_first_is_held`, `an_unwritable_location_is_an_error_not_a_silent_success` (`crates/infrastructure/src/instance_lock.rs`); strict CSP in `production_csp_is_strict_and_dev_csp_only_adds_what_vite_needs` (`src-tauri/tests/adapters.rs`) |
| Preserve the command contract | `commands_preserve_camel_case_arguments_and_snake_case_dtos` (all 14 commands), `snake_case_arguments_are_not_accepted_from_javascript`, `errors_are_presented_as_code_and_message_without_business_rules`; event serialization and the error channel in `src-tauri/tests/adapters.rs`; capabilities (opener restricted to `steam://*`, no unrestricted URL/shell/process permission) in `capabilities_are_minimal_and_open_only_steam_urls` |

Design notes:

- `application` owns the orchestration (`PlayService`) behind narrow traits (`PlayGame`, `ModPlanner`, `Deployer`, `SessionRuntime`) and the ports `SessionLock`, `BackgroundRunner`, `SessionPublisher`; the real services implement them. Infrastructure adds `AtomicSessionLock` and `ThreadRunner`. The Tauri opener and event adapters live in `src-tauri/src/adapters.rs` (`AppHandle` only appears there).
- Review hardening: a still-running game is never cleaned or deployed over (`GameStillRunning`; an inspection failure blocks too and nothing is deleted); the session lock is an RAII guard (a panic or a failed worker spawn marks the cleanup pending before releasing); `recovery_status` keeps the startup result readable; the Tauri commands that write are `async` on the blocking pool and serialized by `AppState::write_lock`; a `FileInstanceLock` (`instance.lock` in the app data dir) is taken before the recovery so only one launcher process runs; the CSP is strict (`devCsp` adds only what Vite needs).
- The lock is held from the Play request until the cleanup finished (released before the final event is published, so the UI can start a new session right after `ended`).
- A failed cleanup (startup, compensation or end of session) keeps the manifest, reports on the `session-error` event and blocks new deploys; the next Play retries the cleanup first and continues only when it succeeds. The lock is released in those cases so the retry is possible.
- `session` payload: `{ state: "started" | "ended" | "never-started", cleaned }`; `session-error` payload: `{ message }`. Command errors serialize as `{ code, message }` (`CommandError`).
- `src-tauri/build.rs` embeds `windows-test.manifest` (Common Controls v6) into the Rust test executables; without it any test that links Tauri fails with `STATUS_ENTRYPOINT_NOT_FOUND` on Windows.
- Manual smoke (not in the gates): with a real game, press Play, confirm the `~mods` files appear, Steam opens and, after closing the game, the files are removed and the `session` event reports the count.

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

Recovery protocol (write-ahead journal in `deployed.json`): the destination is verified absent, journaled in `pending` together with its library source, created exclusively (hard link, copy fallback with `create_new`), then moved to `files`. Cleanup processes `files` + `pending` only (never by name or prefix scan) and retains what it could not process. **Ownership is proven, never assumed:** a journal entry only records an intent. A file under a tracked name is deleted only if `DeployFiles::owns` proves it is the launcher's: the same file as its library source (hard link, via `same-file`) or a byte-identical copy of it. A missing, replaced or user-created file is untracked and left untouched (so a race, a failed untrack save or a crash before creation can never delete a user file). **Authorized directory:** `clean`/`deploy` receive the `~mods` derived from the configured game install; the manifest's directory is canonicalized (resolving `..`, symlinks and junctions) and must equal it, otherwise `UnauthorizedDirectory` is returned and nothing is removed. Accepted limits: a library file that was deleted leaves its deployed copy untracked but in place, and a user file byte-identical to the library source counts as ours.

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

### Issue #9: scenario traceability

Gherkin: `tests/features/frontend/interface.feature`. Each scenario is covered at three levels: the use case against fake ports (`src/application/__tests__/controller.test.tsx`), the screens against a fake controller (`src/ui/__tests__/launcher.test.tsx`) and end to end over the real controller with the in-memory fakes of `src/application/__tests__/fakes.ts` (`src/ui/__tests__/interface.feature.test.tsx`, bound to the Gherkin file with `@amiceli/vitest-cucumber`, one `Scenario` per scenario). Only `src/main.tsx` wires the Tauri adapters (`src/infrastructure`) into the controller.

| Scenario | Tests |
|----------|-------|
| Configure an Unreal folder without Steam | `loads_launcher_state` (application), `settings_accepts_fake_unreal_directory` (ui), e2e "Given the fake folder ..." |
| Use a detected game | `selects_detected_game` (ui), `game is configured and detection works` (application), e2e "Given detection returns a MotoGP with an App ID" |
| Import through the dialog without duplicating | `imports_and_refreshes_library`, `reimport_keeps_single_mod` (application), `library_shows_mod_metadata` (ui), e2e "Given a test pak not yet in the library" |
| Import by drag and drop | `drop_imports_supported_paths` (application), `imports_from_dialog_and_drop` (ui), e2e "Given the Library screen open" |
| Cancel the selection and remove the listener | `cancelled_dialog_does_not_import`, `listeners_are_released_on_dispose` (application), `cancelled_dialog_does_not_import` (ui), e2e "Given an open dialog and active listeners on the screen" (React StrictMode, `activeListeners()` is 0 after unmount) |
| Create a profile and order mods | `saves_profile_mod_order` (application), `profile_order_can_be_changed_with_keyboard` (ui), e2e "Given mods A and B in the library" (saved `mod_ids` B, A and kept after a reload) |
| Rename, delete and protect profiles | `creates_renames_and_deletes_profile`, `vanilla_delete_is_unavailable` (ui), e2e "Given a regular profile and Vanilla" |
| Remove a mod from the library and from profiles | `remove_mod_refreshes_profiles` (application), `library_shows_mod_metadata` (ui), e2e "Given a listed mod referenced in a profile" |
| Warn of an error when playing without an App ID | `play_refuses_without_app_id` (application), `missing_app_id_error_is_visible` (ui), e2e "Given the profile \"Teste\" active ..." (the fake `deployed` stays empty). `~mods` untouched is proven by the Rust application test of #8 |
| Control Play availability | `can_play_rules` (application), `play_is_disabled_without_game_or_during_session` (ui), e2e "Given the app without a configured game" |
| Show session events | `handles_session_events`, `session_error_does_not_announce_cleanup` (application), `session_status_uses_cleaned_count` (ui), e2e "Given a requested session" (started, ended) |
| Show that the game did not open | `handles_session_events` (application), `never_started_status_is_visible` (ui), e2e "Given a requested session" (never-started) |
| Show operation errors | `operation_error_is_exposed` (application), `error_notice_is_announced`, `success_notice_has_status_role` (ui), e2e "Given an operation whose port returns an error" |
| Subscribe before `recovery_status` | `subscribes_before_recovery_status` (application), e2e "Given the launcher starts" (shared event log); a start after dispose (StrictMode remount) keeps a single set of listeners (application "started again after dispose") |
| Carried-over error codes (`game_still_running`, `process_inspection_failed`, `session_thread_failed`, `archive_too_large`) | `has specific messages for the carried-over codes` and `have a message for every error code in both locales` (`src/ui/i18n/__tests__/i18n.test.tsx`) |
| Tauri adapters | `src/infrastructure/__tests__`: `errors_are_normalized` (gateway), `dialog_filters_supported_mod_extensions`, `cancelled_dialog_returns_null`, `single_selection_is_wrapped` (picker), `session_and_drop_adapters_unsubscribe`, `late_unsubscribe_releases_listener`, `drop_ignores_non_drop_events` (events) |
| Internationalization | `defaults to pt-BR`, `uses the given initial locale and switches with setLocale` (i18n), `default_locale_is_pt_br`, `language_select_switches_locale` (ui); the e2e suite runs in `en` |

### Scenario traceability (issue #23)

Gherkin: `crates/application/tests/features/stale-manifest.feature` (in-memory fakes, `tests/support/deploy_fakes.rs` extended with `missing_dirs`, `owns_calls` and two-folder helpers), `crates/infrastructure/tests/features/stale-manifest.feature` (real fs, two game installs in a temp dir) and the `is_mods_dir` rule in `crates/domain/tests/features/deploy-names.feature`. Step modules: `tests/bdd/stale_manifest.rs` (application and infrastructure) and `tests/bdd/deploy_names.rs` (domain). Use-case tests: `crates/application/tests/stale_session.rs`; real-fs tests: `crates/infrastructure/tests/stale_session.rs`.

When the user switches the game install while a session is deployed, the manifest points at the old `~mods` and `clean`/`deploy` refuse it forever (`UnauthorizedDirectory`, unchanged). Recovery is explicit: `DeployService::session` (read-only: `None`, `Current`, `Stale`, `Vanished`), `forget_stale_session` (clears the manifest, never calls `owns`/`remove`, refuses the current session with `SessionNotStale`) and `clean_stale_session` (same manifest validation and the same removal loop as `clean`, via the shared `remove_tracked`). Safety: the confirmation must canonicalize to exactly the recorded folder, that folder must be named `~mods` (`is_mods_dir`), and every deletion still requires `DeployFiles::owns`.

| Scenario | Test |
|----------|------|
| Detect a manifest of a previous game install | `session_is_stale_with_the_old_folder_and_counts_files_and_pending`, `session_is_read_only` (application), `a_manifest_of_another_install_is_reported_stale_without_changes` (infra) + cucumber in both features |
| Forget the old session | `forget_clears_a_stale_manifest_without_touching_any_file`, `forget_clears_a_vanished_manifest`, `forget_with_an_empty_manifest_is_a_no_op` (application), `forget_clears_the_manifest_and_deletes_nothing` (infra) + cucumber |
| Clean the old folder after confirmation | `confirmed_clean_removes_owned_files_of_the_old_folder_and_clears_the_manifest` (application), `confirmed_clean_removes_only_our_files_of_the_old_folder` (infra) + cucumber |
| A foreign file in the old folder is never deleted | `confirmed_clean_leaves_a_foreign_file_untouched` (application), `confirmed_clean_removes_only_our_files_of_the_old_folder` (infra) + cucumber |
| A file modified since the deploy is not deleted | `confirmed_clean_keeps_a_modified_file_and_untracks_it` (application), `a_tracked_file_replaced_by_the_user_is_kept_and_untracked` (infra) + cucumber |
| The confirmation must name the recorded folder | `clean_with_a_wrong_confirmation_is_refused_and_removes_nothing` (application), `a_confirmation_naming_another_folder_is_refused`, `a_confirmation_with_dot_dot_that_resolves_to_the_recorded_folder_is_accepted` (infra) + cucumber |
| Refuse to clean a folder that is not a ~mods folder | `clean_refuses_a_recorded_folder_that_is_not_named_mods` (application), `a_recorded_folder_not_named_mods_is_refused` (infra), `mods_dir_is_recognized_by_its_last_component`, `other_directories_are_not_mods_dirs` (domain) + cucumber in all three features |
| The old folder no longer exists | `session_is_vanished_when_the_recorded_folder_is_gone`, `clean_of_a_vanished_folder_clears_the_manifest` (application), `a_vanished_old_folder_is_cleared_without_an_error`, `session_is_none_current_or_vanished_as_the_manifest_dictates` (infra) + cucumber |
| A manifest of the current install keeps the normal behaviour | `session_is_current_when_the_manifest_points_at_the_authorized_dir`, `forget_refuses_the_current_session_and_keeps_the_manifest`, `clean_of_the_current_session_behaves_like_the_normal_clean`, `normal_clean_and_deploy_still_refuse_a_stale_manifest` (application), `forget_refuses_the_current_install_and_keeps_the_manifest`, `clean_stale_session_on_the_current_install_behaves_like_clean` (infra) + cucumber |
| Deploy works again after recovering | `deploy_works_again_after_forgetting_the_old_session` (application and infra) + cucumber |
| A partially failed cleanup of the old folder can be retried | `partial_failure_keeps_the_entry_against_the_old_folder_for_a_retry` (application, includes the retry) + cucumber (application only) |
| Tampered manifest removes nothing | `clean_rejects_tampered_names_before_removing_anything`, `clean_rejects_an_empty_recorded_directory` (application) |

### Issue #34: scenario traceability

Gherkin: `tests/features/frontend/stale-session.feature` (UI, e2e over the in-memory fakes) and `crates/application/tests/features/stale-session-recovery.feature` (9 scenarios, steps in `tests/bdd/stale_session_recovery.rs`). Frontend tests: `src/ui/__tests__/stale-session.feature.test.tsx` (bound to the Gherkin file; real App over the real controller), `src/ui/__tests__/staleSession.test.tsx` (screen tests over the fake controller), the `Given a stale deploy session` block of `src/application/__tests__/controller.test.tsx` and the gateway cases of `src/infrastructure/__tests__/tauriGateway.test.ts`. Rust: `crates/application/tests/stale_recovery.rs` and the IPC contract `src-tauri/tests/stale_session_contract.rs`.

Design note: the recovery rules live in `PlayService` (session lock, `GameRunning`, a `Blocked` recovery becomes `Ready`), and the Tauri commands are thin wrappers. IPC contract: `deploy_session` (read, `{ state, mods_dir, tracked }`), `forget_stale_session` and `clean_stale_session { confirmedDir }` (camelCase argument only; returns the number of files removed). The confirmation is a UI step: the controller sends the folder shown in the notice as `confirmedDir`, and the backend refuses anything that is not exactly the recorded folder.

| Scenario | Test |
|----------|------|
| The user forgets the old session | `forget_stale_session` (application), `forget_button_calls_the_controller` (ui), e2e "When the user chooses \"Forget the old session\" ..."; Rust `forgetting_a_stale_session_empties_the_manifest_and_keeps_the_files`, `play_succeeds_after_forgetting_a_stale_session` (application), IPC `forget_stale_session_clears_the_manifest_and_keeps_the_files`, `play_works_after_forgetting_the_stale_session` |
| The user cleans the old folder | `clean_stale_session` (application), `clean_asks_for_confirmation_naming_the_folder_before_calling`, `confirming_calls_clean` (ui), e2e "When the user confirms cleaning the old folder ..."; Rust `cleaning_with_the_confirmed_folder_removes_only_owned_files`, IPC `clean_stale_session_accepts_the_camel_case_folder_and_removes_owned_files` |
| Play is blocked while a stale session is pending | `play_blocked_while_stale` (application), `stale_notice_names_the_folder_and_the_file_count`, `shows_the_folder_in_pt_BR_too` (ui), e2e "Then a notice names the old folder and Play is disabled" |
| Cancelling the confirmation keeps everything | `cancelling_returns_to_the_notice_without_calling_anything`, `buttons_are_disabled_while_busy` (ui), e2e "... and cancels Then nothing is removed ..." |
| Cleaning is refused when the confirmed folder does not match | `clean_refused_keeps_notice`, `confirmed_dir_mismatch` (application), e2e "When cleaning is confirmed with a different folder ..."; Rust `cleaning_with_a_different_folder_is_refused_and_removes_nothing`, IPC `clean_stale_session_with_a_wrong_folder_is_refused_and_removes_nothing`, `clean_stale_session_rejects_snake_case_arguments` |
| Switching the game folder makes the session stale | `switch_game_makes_stale` (application), e2e "When the user configures another game folder Then the stale notice appears" |
| A vanished old folder needs no recovery | `vanished_needs_no_recovery` (application), e2e "Given the manifest points to a folder that no longer exists"; Rust `deploy_session_reports_none_current_stale_and_vanished`, IPC `deploy_session_dto_has_the_documented_shape_for_every_state` |
| A stale manifest at startup unblocks Play once resolved | `startup_recovery_failed_suppressed` (application), e2e "Given the startup recovery failed because of the stale manifest"; Rust `forgetting_after_a_stale_startup_failure_unblocks_play`, `cleaning_after_a_stale_startup_failure_unblocks_play_and_reports_the_count`, `a_refused_recovery_keeps_the_blocked_state` |
| The current session cannot be forgotten | `current_cannot_be_forgotten` (application); Rust `forgetting_the_current_session_is_refused_and_keeps_the_manifest` (application and IPC) |
| Recovery is refused while a session is active | `active_session_refuses` (application); Rust `recovery_is_refused_while_a_session_holds_the_lock`, `recovery_is_refused_while_the_previous_game_is_still_running`, IPC `recovery_is_refused_while_a_session_is_active`; `recovery_releases_the_lock_and_does_not_block_play` |
| Recovery needs a configured game | `no_game_no_backend_call` (application); Rust `recovery_requires_a_configured_game`, `deploy_session_requires_a_configured_game`, IPC `the_recovery_commands_need_a_configured_game` |
| Notices and messages | `stale_notices_have_texts`, `uses_the_singular_for_one_file` (ui), `have a message for every error code in both locales` and the locale key parity tests (`src/ui/i18n/__tests__/i18n.test.tsx`) |

### Issue #10: scenario traceability

Gherkin: `tests/features/frontend/auto-update.feature` (frontend) and `src-tauri/tests/features/updater.feature` (Rust cucumber runner `updater_support`, registered in `src-tauri/tests/features.rs`). Each frontend scenario is covered by the domain rule (`src/domain/__tests__/update.test.ts`), the use case against `FakeUpdateService` (`src/application/useCases/__tests__/checkForUpdate.test.ts`, `src/application/__tests__/selectSessionClean.test.ts`), the Tauri adapter with the SDK mocked at the boundary (`src/infrastructure/__tests__/tauriUpdateService.test.ts`), the banner (`src/ui/__tests__/UpdateBanner.test.tsx`) and end to end over the real launcher controller, the real update controller and the real UI (`src/ui/__tests__/autoUpdate.feature.test.tsx`, bound to the Gherkin file with `@amiceli/vitest-cucumber`, one `Scenario` per scenario; the production-flow scenario only checks that the Rust feature still states the same rules). Only `src/main.tsx` wires `createTauriUpdateService()` into the update controller and triggers the startup check.

| Scenario | Tests |
|----------|-------|
| Find and install a newer version | `checkForUpdate_returns_new_release`, `download_progress_is_forwarded`, `restart_only_after_successful_install` (application), `updater_adapter_checks_downloads_installs_and_restarts` (infrastructure), `shows_new_version_notes_and_update_button`, `shows_determinate_or_indeterminate_progress` (ui), e2e "Scenario: Find and install a newer version" (progress visible, install completes before restart). The real restart on the new version is validated in #11 |
| Already on the latest version | `checkForUpdate_returns_none_when_current` (application), `hides_banner_when_up_to_date` (ui), e2e "Scenario: Already on the latest version" |
| Do not install an older version | `isNewerVersion` tests (domain, `1.10.0` vs `1.9.0` is numeric), `older_release_is_not_offered` (application), `version_check_uses_updater_semantics` (infrastructure), e2e "Scenario: Do not install an older version" |
| Keep the launcher usable offline | `check_failure_does_not_block_launcher` (application), e2e "Scenario: Keep the launcher usable offline" (notice visible, Library, Profiles and Settings still reachable, no restart) |
| Refuse a package with an invalid signature | `invalid_signature_error_is_propagated` (infrastructure), "Reject an unsigned or tampered update package" (`unsigned_or_tampered_update_is_rejected`, Rust cucumber, real minisign verification of the fixtures), e2e "Scenario: Refuse a package with an invalid signature" |
| Handle an installation failure | `install_failure_does_not_restart` (application), `update_error_is_accessible` (ui), e2e "Scenario: Handle an installation failure" |
| Avoid simultaneous installs | `concurrent_update_is_rejected` (application), e2e "Scenario: Avoid simultaneous installs" (double click and a direct second `install()` give one download) |
| Protect a game session | `active_or_unclean_session_blocks_install` (application), `selectSessionClean` tests, `session_state_disables_update_action` (ui), e2e "Scenario: Protect a game session" (session event and recovery `game-running`; a direct `install()` is refused with `session`) |
| Publish the feed only through the production flow | Rust cucumber only: "Publish the update feed only after validation on main" (`release_job_requires_successful_validation`), "Configure the updater with the production public key" (`updater_config_contains_public_key_and_endpoint`), "Grant the window only the updater permissions it needs" (`updater_capabilities_are_minimal`) (`src-tauri/tests/features/updater.feature`); the bound frontend scenario only asserts that this Rust feature still states the same rules |

The scenario "Configure the updater with the production public key" stays red on purpose until the real signing key pair exists and its public key replaces the placeholder in `plugins.updater.pubkey` (external setup, see [auto-update.md](auto-update.md)). The real two-version update (install, restart on the new version) is validated in issue #11.

### Issue #53: scenario traceability

Gherkin: `tests/features/frontend/about.feature`, bound end to end by `src/ui/__tests__/about.feature.test.tsx` (`@amiceli/vitest-cucumber`, one `Scenario` per scenario) over the real launcher controller, the real update controller (`createUpdateController` with `FakeUpdateService`, extended in the test so `check()` can be held open) and an inline fake `AppInfo`. The version is read by `readAppVersion` (`src/application/useCases/__tests__/readAppVersion.test.ts`) through `createTauriAppInfo` (`src/infrastructure/__tests__/tauriAppInfo.test.ts`, `@tauri-apps/api/app` mocked at the boundary). Only `src/main.tsx` wires `createTauriAppInfo()` into the `App`.

| Scenario | Tests |
|----------|-------|
| The status bar shows the launcher version | "returns the trimmed version" (application), "resolves the SDK version" (infrastructure), e2e "Scenario: The status bar shows the launcher version" (`v0.1.0` inside the status bar game block) |
| Settings About shows the launcher version | e2e "Scenario: Settings About shows the launcher version" (About link current, version row labelled "Launcher version"); "About shows the version and no update check without an updater" (trimmed version, no check row) |
| The version stays hidden when it cannot be read | "returns null and warns when the port rejects", "returns null and warns when getVersion throws synchronously", "returns null for an empty or whitespace version" (application), "propagates a rejection" (infrastructure), e2e "Scenario: The version stays hidden when it cannot be read" (no caption, Play enabled, About and Library reachable) |
| Checking manually when already up to date | e2e "Scenario: Checking manually when already up to date" (button disabled as "Checking…" while the held check runs, then the polite status says up to date) |
| Checking manually finds a newer version | e2e "Scenario: Checking manually finds a newer version" (status says "Version 0.2.0 is available"; "Install update" is an accent outline, downloads and restarts through the controller) |
| A manual check that fails shows a translated error | e2e "Scenario: A manual check that fails shows a translated error" (pt-BR network message, raw updater text absent from the page) |
| A failed background check does not claim the launcher is up to date | e2e "Scenario: A failed background check does not claim the launcher is up to date" (controller in `none` after the silent check, About shows neither "up to date" nor an error) |
| Install from About is blocked during a game session | e2e "Scenario: Install from About is blocked during a game session" (install disabled, `aria-describedby` = the session hint, no download) |

### Scenario traceability (issue #56)

Storage usage in Settings. Gherkin: `crates/domain/tests/features/storage-classification.feature`, `crates/application/tests/features/storage.feature`, `crates/infrastructure/tests/features/storage-scan.feature` (cucumber runners `storage_classification`, `storage`, `storage_scan`) and the IPC contract in `src-tauri/tests/storage_contract.rs`. Domain and application use fakes only; the infrastructure scenarios use temp dirs; the IPC tests use the Tauri mock runtime and a fake folder opener. No new `AppError` variant or error code: failures reuse `read_failed` and `task_failed`.

#### Rust

| Scenario | Tests |
|----------|-------|
| Imported mods are counted as mods | `library_directory_is_mods_in_any_case`, `a_library_file_is_other` (domain), cucumber "Imported mods are counted as mods" |
| Launcher state files are counted as launcher data | `state_files_and_their_tmp_siblings_are_launcher_data` (domain), cucumber "Launcher state files are counted as launcher data" |
| Anything else is counted as other | `other_entries_are_other` (domain), cucumber "Anything else is counted as other" |
| The total is the sum of the categories | `usage_keeps_every_category_in_order_even_when_empty`, `usage_sums_entries_per_category`, `sums_saturate_instead_of_overflowing` (domain), cucumber "The total is the sum of the categories" |
| Usage is grouped per category | `groups_and_sums_entries` (application), cucumber "Usage is grouped per category" (accented path), IPC `get_storage_usage_returns_the_dto`, `storage_usage_dto_keeps_the_category_order_and_snake_case_fields` |
| An empty data folder reports zero in every category | cucumber "An empty data folder reports zero in every category" |
| Free space that cannot be read does not fail the usage | `unknown_free_space_is_none` (application), cucumber "Free space that cannot be read does not fail the usage" |
| A data folder that cannot be read is an error | `a_scan_failure_is_an_error` (application), `a_root_that_is_a_file_is_a_read_error` (infrastructure), cucumber "A data folder that cannot be read is an error" |
| Open folder opens the data folder | `opens_only_the_data_folder_and_reports_failures` (application), cucumber "Open folder opens the data folder", IPC `open_data_folder_opens_only_the_data_folder` |
| Sizes are measured recursively in a path with accents | `measures_children_recursively` (infrastructure), cucumber "Sizes are measured recursively in a path with accents" |
| A missing data folder is empty, not an error | `a_missing_root_is_empty_and_still_reports_free_space` (infrastructure), cucumber "A missing data folder is empty, not an error" |
| Links are not followed | cucumber "Links are not followed" (junction on Windows via `mklink /J`, symlink elsewhere; skipped with a printed note when the OS refuses the link), `symlinks_are_not_followed` (infrastructure, unix) |
| Measuring does not wait for a running mutation | IPC `measuring_does_not_wait_for_a_running_mutation` (holds `write_lock` while invoking `get_storage_usage`) |

#### Frontend

Gherkin: `tests/features/frontend/storage.feature`, bound by `src/ui/__tests__/storage.feature.test.tsx` (real UI over the fake controller, measurement resolved, pending or rejected by the test). Edge cases in `src/ui/__tests__/StorageSection.test.tsx`; pure rules in `src/domain/__tests__/storage.test.ts`; controller pass-through in `src/application/__tests__/storage.test.ts`; DTO mapping in `src/infrastructure/__tests__/tauriGateway.test.ts`.

| Scenario | Tests |
|----------|-------|
| The section shows the data folder, totals and the breakdown | vitest-cucumber "The section shows the data folder, totals and the breakdown" (accented path, `role="meter"` per category, `<1%` share), `storageBreakdown` / `storageShare` (domain), gateway "DTO is mapped to camelCase" |
| Open folder asks the backend to open the data folder | vitest-cucumber "Open folder asks the backend to open the data folder", "When opening the folder fails Then an inline message is shown" (StorageSection), controller "the gateway is called with no path", gateway `open_data_folder` case |
| Refresh measures again without blocking the screen | vitest-cucumber "Refresh measures again without blocking the screen", "When an older answer arrives after a newer one Then the older answer is ignored", "When the first measurement is running Then skeletons are shown" (StorageSection) |
| Unknown free space is shown as unavailable | vitest-cucumber "Unknown free space is shown as unavailable", gateway "free space is kept as a number" |
| A failed measurement shows an inline error with retry | vitest-cucumber "A failed measurement shows an inline error with retry", "When Retry succeeds after a failure", "When a refresh fails after a successful measurement" (StorageSection), controller "no global error is set" |
| The section is translated | vitest-cucumber "The section is translated" (pt-BR labels and `1,5 GB`) |

### Load order and conflicts (#55)

The load order is `Profile.mod_ids` (index 0 loads first, the last one wins). Gherkin (Rust side): `crates/domain/tests/features/load-order.feature`, `crates/application/tests/features/load-order.feature` and `crates/infrastructure/tests/features/profile-persistence.feature`. The frontend scenarios live in `tests/features/frontend/load-order.feature`. Limitation: only file-name conflicts are detected, not asset conflicts inside paks.

| Scenario | Tests |
|----------|-------|
| Later mod in the load order wins a conflict | `later_mod_wins_and_path_uses_the_winner_spelling` (domain), domain cucumber "Later mod in the load order wins a conflict" |
| Reordering changes the winner | `reordering_changes_the_winner`, `three_mods_keep_load_order` (domain), domain cucumber "Reordering changes the winner" |
| Mods without shared files do not conflict | `disjoint_files_do_not_conflict`, `a_mod_shipping_the_name_twice_counts_once` (domain), domain cucumber |
| Missing and duplicate mods are ignored | `missing_and_duplicate_ids_are_ignored` (domain), domain cucumber |
| The load order has no duplicates | `set_load_order_dedupes_keeping_first_occurrence`, `normalize_load_order_keeps_unique_input_untouched` (domain), domain cucumber |
| Saving the load order persists it normalized | `set_mods_dedupes_keeping_first_occurrence` (application), application cucumber "Saving the load order persists it normalized" |
| Listing conflicts for a candidate order | `list_conflicts_reads_the_library_and_follows_the_candidate_order` (application), application cucumber, IPC `list_conflicts_takes_camel_case_mod_ids_and_returns_snake_case_dtos`, `list_conflicts_rejects_snake_case_arguments` |
| Deploy follows the load order | `resolve_profile_mods_deploys_a_repeated_id_once` (application), application cucumber "Deploy follows the load order" |
| A legacy profiles file keeps its implicit order | `legacy_file_keeps_its_order_and_unknown_ids` (infrastructure), infrastructure cucumber "A legacy profiles file keeps its implicit order" |
| Duplicate IDs in a legacy file collapse to the first occurrence | `duplicate_mod_ids_collapse_to_the_first_occurrence_on_load` (infrastructure), infrastructure cucumber |
| Reorder the load order with the keyboard | `tests/features/frontend/load-order.feature` bound by `src/ui/__tests__/load-order.feature.test.tsx` (Enter on "Move Beta up", focus kept on the moved mod, save sends `["beta", "alpha"]`) |
| The conflicts panel previews the draft order | `load-order.feature.test.tsx` (panel lists the path and the mods in load order, the winner follows the unsaved draft, nothing is saved) |
| No conflicts | `load-order.feature.test.tsx` (status reads "No conflicts"; with one mod left no preview request is made) |
| Library rows show a conflict badge for the active profile | `load-order.feature.test.tsx` ("Overridden" / "Wins" badges with their tooltip and accessible sentence, no badge without conflicts) |
