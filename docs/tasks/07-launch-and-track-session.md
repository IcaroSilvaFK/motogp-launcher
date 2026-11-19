---
title: "Launch the game and track the session"
labels: [feature, domain, application, infrastructure]
depends_on: ["05"]
branch: feat/7-launch-and-track-session
---

## Context

Adapts Task 7. Separate the state machine from the real process observation and URL opening. This issue prepares the session components; orchestration with deploy and the Tauri commands come in issue 08.

## Scope (per layer)

- **Domain:** implement `launch_url`, `WatchEvent { Started, Ended, NeverStarted }`, `WatchState` and `watch_step` as pure rules. With an App ID, generate `steam://rungameid/<id>`; without it, return "game App ID not found; open the game from Steam once". The first positive observation yields Started; two consecutive absences after that yield Ended; a positive observation resets the count. Never observed up to 120 s inclusive yields NeverStarted. Terminal events are not repeated.
- **Application:** define the Steam opening, process inspection and time/poll ports as the use case needs. Tracking uses a 2 s interval (`POLL`), receiving observations from the adapters. Tests use a fake clock and processes, with no real 120 s sleeps.
- **Infrastructure:** implement `sysinfo` inspection and a real clock. Consider a process as the game's only if its executable is inside the game root, case-insensitively on Windows and respecting path components: `MotoGP25Backup` is not inside `MotoGP25`. An unavailable executable does not prove the game is running. The real opening via the opener plugin will be wired in the bootstrap of issue 08; keep the port and the URL contract here.
- Return application events without Tauri serialization; serialization and emission to the UI belong to the interface.

## Out of scope

Deploy/clean, Tauri commands, UI, fixed App ID, detection by window title and polling in the frontend.

## Acceptance criteria (Gherkin)

Files: `crates/domain/tests/features/session-state.feature`, `crates/application/tests/features/session-tracking.feature` and `crates/infrastructure/tests/features/processes.feature`; `cucumber` runners. Use controlled process lists and time in the tests, without starting a real game.

```gherkin
Feature: Identify the start and end of the session
  Scenario: Build the URL with a detected App ID
    Given a game with App ID 123
    When I prepare its launch
    Then I get "steam://rungameid/123"

  Scenario: Refuse to launch without an App ID
    Given a game without an App ID
    When I prepare its launch
    Then I get "game App ID not found; open the game from Steam once"

  Scenario: End after two consecutive absences
    Given the first poll finds the game and emits Started
    When the next poll does not find the process
    Then no end is emitted
    When the following poll also does not find the process
    Then Ended is emitted exactly once

  Scenario: Reset absences when the process reappears
    Given the poll sequence present, absent, present, absent
    When I advance the state machine
    Then Started is emitted on the first presence
    And Ended is not emitted

  Scenario: Report a game that never started
    Given no poll has found the process
    When the elapsed time reaches 119 seconds
    Then no end is emitted
    When the time reaches 120 seconds
    Then NeverStarted is emitted exactly once

  Scenario: Observe only executables inside the root
    Given a game root "C:/Jogos/MotoGP25"
    And executables at "c:/jogos/motogp25/Binaries/jogo.exe" and "C:/Jogos/MotoGP25Backup/outro.exe"
    When I classify the processes
    Then only the first is considered a game process

  Scenario: Use the defined tracking interval
    Given a controlled clock and process inspector
    When I track a session
    Then the queries are scheduled at 2-second intervals
```

## TDD tests (write first)

Preserve the five plan names in **crates/domain**, with no Tauri/sysinfo/real time:

- `launch_url_uses_app_id`.
- `launch_url_without_app_id_errors`.
- `started_then_ended_after_two_missing_polls`.
- `flapping_resets_missing_counter`.
- `never_started_after_120s`.
- **Domain:** add `terminal_watch_event_is_not_repeated`.
- **Application:** add `watcher_polls_every_two_seconds` with a fake clock/inspector.
- **Infrastructure:** add `process_path_comparison_is_case_insensitive`, `sibling_directory_is_not_game_root` and `missing_process_executable_is_ignored`, using controlled data for the adapter, without depending on the CI machine's processes.

## Definition of Done

- [ ] Red → Green → Refactor evidenced; five original names kept; no core test depends on real processes/sleep.
- [ ] Each scenario has a `.feature` and a cucumber test in the indicated crate.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` and Vitest scenarios green.
- [ ] Gate 6: Cargo dependencies inward and boundaries green; sysinfo/opener/Tauri outside the core.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: time and path limits proven; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved before the PR.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
