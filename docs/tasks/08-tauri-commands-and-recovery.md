---
title: "Integrate Tauri commands and session recovery"
labels: [feature, application, infrastructure, interface]
depends_on: ["03", "04", "05", "06", "07"]
branch: feat/8-tauri-commands-and-recovery
---

## Context

Adapts Task 8. The old plan put profile resolution and session orchestration in `commands.rs`; they move to application. Tauri commands only convert DTOs, call use cases and map responses/errors.

## Scope (per layer)

- **Application:** implement the use cases still needed to configure the game, the active profile, multiple import, reference removal and start/recover a session. Move `resolve_profile_mods` to application: preserve the profile order and ignore IDs missing from the library. Validate the game and App ID **before** deploying mods; without an App ID, return the error from issue 07 without touching `~mods`.
- **Orchestration:** block concurrent sessions from the launch request until cleanup finishes, including while the game is still starting. Resolve the profile, deploy the mods, open Steam and track events. On Ended/NeverStarted, clean and publish `session` with the state and the real number of files removed; on Started, only publish. Deploy/opener failures trigger compensation and release the lock when it is safe; cleanup failures preserve the manifest/pending items and are reported, without announcing a false success.
- **Infrastructure:** compose the already-delivered repositories and adapters; provide the session lock implementation, background execution and event publishing through ports, without bringing Tauri into application. Implement the Steam opening bridge with the plugin in the interface, where `AppHandle` is available.
- **Interface:** create a thin `src-tauri/src/commands.rs` and the composition root in the bootstrap. Resolve `app.path().app_data_dir()` and inject the adapters/state; before accepting new operations, run the manifest recovery use case. Record the amount cleaned; a recovery error is visible and blocks new deploys until it is resolved.
- **IPC contract:** preserve the commands `get_config`, `detect_games`, `set_game_dir(path)`, `get_game`, `list_mods`, `import_mods(paths)`, `remove_mod(id)`, `list_profiles`, `create_profile(name)`, `rename_profile(id, name)`, `delete_profile(id)`, `set_profile_mods(id, modIds)`, `set_active_profile(id)` and `play(profileId)`. Returned DTOs keep snake_case; JS arguments use camelCase. `import_mods` stops at the first failure and reports an error; previously completed imports remain visible after a reload. `remove_mod` removes references in all profiles, reporting failures without simulating completion.
- **Consistency:** `set_game_dir` inspects before saving; `get_game` returns None only if not configured, without hiding an inspection error for a configured path. Validate the active profile; when deleting the active profile, restore the Vanilla selection. Protect Vanilla via IPC too.
- **Events/plugins:** serialize `SessionEvent { state, cleaned }`, with states `started`, `ended`, `never-started`; failures follow a visible error channel and not a false completion event. Register dialog/opener and minimal capabilities, including opening restricted to `steam://*`, without unrestricted URL/process permission.

## Out of scope

React components, updater, a new deploy engine or rules inside commands. Do not duplicate the watcher in the interface.

## Acceptance criteria (Gherkin)

Main file: `crates/application/tests/features/commands-and-recovery.feature`, `cucumber` runner, with fake ports. IPC/bootstrap contracts: `src-tauri/tests/features/tauri-interface.feature`, `cucumber` runner with a Tauri test environment; no real game required in the gates.

```gherkin
Feature: Expose use cases and recover sessions through the launcher
  Scenario: Resolve mods in order ignoring missing references
    Given a profile with IDs "b", "gone", "a" and a library with A and B
    When I resolve the profile mods
    Then I get B and A in that order

  Scenario: Recover a previous deployment on startup
    Given a pending manifest and a manual file in ~mods
    When the launcher starts
    Then the recovery cleans only the files deployed by the launcher
    And records the amount removed before accepting a new session

  Scenario: Prevent a simultaneous session
    Given a session starting or running
    When I receive another Play request
    Then I return "the game is already running"
    And I neither deploy nor open the game again

  Scenario: Validate prerequisites before deploy
    Given a game not configured or without an App ID
    When I receive the Play request
    Then I return the corresponding error
    And no file or directory is created in ~mods
    And a future attempt remains possible

  Scenario: Finish a session with cleanup and event
    Given a session with deployed files
    When the watcher signals Ended or NeverStarted
    Then the cleanup runs
    And the session event contains the state and the amount actually removed
    And a new session can be started after the complete cleanup

  Scenario: Emit start without cleaning prematurely
    Given a session with deployed mods
    When the watcher signals Started
    Then I emit session with state "started"
    And the files remain deployed

  Scenario: Compensate a failure to open Steam
    Given mods deployed for a session attempt
    When the opener adapter returns an error
    Then I try to clean only the files of that session
    And I report the error without keeping the lock after a successful compensation

  Scenario: Do not hide a recovery failure
    Given the manifest cleanup fails on startup or at the end
    When the operation returns
    Then I report the failure and retain the recoverable pending items
    And I do not announce a clean session nor allow an unsafe new deploy

  Scenario: Keep profiles consistent when removing data
    Given a mod referenced in profiles and a regular active profile
    When I remove the mod and delete the active profile
    Then the references to the mod are removed from all profiles
    And the active profile becomes Vanilla

  Scenario: Preserve the command contract
    Given the registered commands and controlled application ports
    When I send camelCase arguments through IPC
    Then each command forwards the expected operation
    And responses use the documented snake_case fields
    And errors are presented without business rules in the command
```

## TDD tests (write first)

- Preserve `resolve_profile_mods_keeps_order_and_skips_missing` in **crates/application**, and not in commands.rs as in the old plan.
- **Application**, unit tests with fakes: `play_without_game_does_not_deploy`, `play_without_app_id_does_not_deploy`, `concurrent_play_is_rejected`, `startup_recovers_previous_deployment`, `ended_session_cleans_and_emits_count`, `never_started_session_cleans_and_releases_lock`, `started_event_does_not_clean`, `opener_failure_rolls_back_deployment`, `cleanup_failure_keeps_recovery_pending`, `remove_mod_clears_profile_references` and `deleting_active_profile_selects_vanilla`.
- **Interface/src-tauri**, contract tests: `commands_preserve_camel_case_arguments_and_snake_case_dtos`, covering each listed command with fakes, and `setup_runs_recovery_before_accepting_play`. They are boundary tests, with no rule duplicated in the command.
- **Application:** `batch_import_stops_at_first_error_and_keeps_completed_imports`, checking the plan's partial-failure contract.

## Definition of Done

- [ ] Red → Green → Refactor evidenced and original name preserved in the correct layer; 35 original tests of issues 02–08 still present/passing.
- [ ] All scenarios have a `.feature` and cucumber in the matching crate; IPC and permissions also verified on Windows.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` in the crates involved and Vitest scenarios green.
- [ ] Gate 6: thin commands; domain with no external production deps; application depends only on domain; frontend boundaries green.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria proven, including failures/compensation; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved and Codex review applied; no session without an App ID leaves the game modified.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
