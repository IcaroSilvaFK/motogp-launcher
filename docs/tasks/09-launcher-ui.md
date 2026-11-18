---
title: "Build the launcher UI"
labels: [feature, frontend]
depends_on: ["08"]
branch: feat/9-launcher-ui
---

## Context

Adapts Task 9. Deliver Library, Profiles, Settings and session control with React + TypeScript. Replace the plan's monolithic `src/api.ts` with separate pure types, ports, use cases and adapters.

## Scope (per layer)

- **Frontend/domain:** define `Config`, `GameInfo`, `ModEntry`, `Profile` and `SessionEvent` without the Tauri SDK; keep response fields in snake_case (`game_dir`, `mods_dir`, `app_id`, `mod_ids`, `size_bytes`, `active_profile`) per the IPC of issue 08.
- **Frontend/application:** define ports for launcher operations, file/folder selection and session/drop events. Implement use cases/hooks to load library/profiles/game, import/remove, edit order, select a profile and play. Inject adapters; handle errors and loading/session states with fakes in tests. Reload the data after mutations, including to reflect previously completed imports when a batch import fails. Dispose listeners on unmount.
- **Frontend/infrastructure:** implement a typed adapter per command with `invoke`, camelCase arguments and snake_case responses. Isolate `listen("session")`, `getCurrentWebview().onDragDropEvent` and plugin-dialog in this layer. In the import dialog, allow multiple `.pak/.utoc/.ucas/.zip`; in the settings dialog, select a directory. Cancelling is not an error and does not trigger an import.
- **Frontend/ui:** put components/layout in `src/ui`, including the Library, Profiles and Settings screens and the App. Sidebar with navigation, active-profile selector and Play disabled without a configured game or while a session is starting/active. Show the states "Running…", "Session ended — N files removed" and "The game did not open within 2 min; mods removed" from the event, without announcing cleanup in case of an error.
- **Library:** show name, size and file count, import via dialog/drop and remove with a reload of the affected profiles.
- **Profiles:** create/rename/delete, present mod checkboxes and accessible ↑/↓ controls; persist in the chosen order. Vanilla offers no deletion, in addition to the backend protection already delivered.
- **Settings:** show the game's name, `mods_dir` and App ID; list detections with "Use this" and offer "Choose folder…". A valid folder without an App ID can be configured, but Play must receive the backend error without deploying files.
- **Composition and accessibility:** `src/main.tsx` injects adapters into the use cases; ui components never import infrastructure. Inputs/buttons have accessible names, keyboard navigation and usable focus; an error notice at the top with an accessible announcement. Remove the template's presentation.

## Out of scope

Auto-update (issue 10), mod editor, online browsing, process polling in React and business rules duplicated in the UI. Do not create a `src/api.ts` mixing types, SDK and rules.

## Acceptance criteria (Gherkin)

Source of the scenarios: `tests/features/frontend/interface.feature`. In the frontend, per the workflow, run them as `describe("Given…")` / `it("When… Then…")` in Vitest + Testing Library: `src/ui/__tests__/launcher.test.tsx` and application tests. This `.feature` is the traceable specification; do not add Rust cucumber to test React. The corresponding backend behaviors remain in the cucumber runners of the earlier issues.

```gherkin
Feature: Use the launcher through the interface
  Scenario: Configure an Unreal folder without Steam
    Given the fake folder "C:/temp/MotoGPFake/MotoGPFake/Content/Paks"
    When I choose "C:/temp/MotoGPFake" in Settings
    Then I see mods_dir ending in "Content/Paks/~mods"
    And I see the App ID as not found

  Scenario: Use a detected game
    Given detection returns a MotoGP with an App ID
    When I click "Use this"
    Then the game configuration is persisted
    And the current game is updated on screen

  Scenario: Import through the dialog without duplicating
    Given a test pak not yet in the library
    When I select it in Import
    Then the library shows its name, size and file count
    When I import the same pak again
    Then there is still a single entry for the mod

  Scenario: Import by drag and drop
    Given the Library screen open
    When I receive a drop event with supported files
    Then the paths are sent to the import use case
    And the list is reloaded

  Scenario: Cancel the selection and remove the listener
    Given an open dialog and active listeners on the screen
    When I cancel the dialog and then unmount the screen
    Then no import is requested
    And the listeners are released without duplicated callbacks

  Scenario: Create a profile and order mods
    Given mods A and B in the library
    When I create the profile "Teste", check the mods and move B before A
    Then I save mod_ids in the order B, A
    And the order remains after reloading the profile

  Scenario: Rename, delete and protect profiles
    Given a regular profile and Vanilla
    When I rename the regular profile and then delete it
    Then the list reflects both changes
    And Vanilla offers no delete action

  Scenario: Remove a mod from the library and from profiles
    Given a listed mod referenced in a profile
    When I remove it from the Library
    Then it no longer appears in the library nor in the profile selection

  Scenario: Warn of an error when playing without an App ID
    Given the profile "Teste" active and a game configured without an App ID
    When I click Play
    Then I see a notice containing "game App ID not found"
    And no file is created in ~mods

  Scenario: Control Play availability
    Given the app without a configured game
    Then Play is disabled
    When I configure a game and request to start a session
    Then Play stays disabled during start and execution

  Scenario: Show session events
    Given a requested session
    When I receive session with state "started"
    Then I see "Running…"
    When I receive session with state "ended" and cleaned 2
    Then I see "Session ended — 2 files removed"
    And Play becomes available again

  Scenario: Show that the game did not open
    Given a requested session
    When I receive session with state "never-started" after the cleanup finishes
    Then I see "The game did not open within 2 min; mods removed"

  Scenario: Show operation errors
    Given an operation whose port returns an error
    When the operation finishes
    Then I see the error in the accessible notice at the top
    And I do not see a false success confirmation
```

## TDD tests (write first)

The plan did not name frontend unit tests. Write before the components/use cases:

- **Frontend/application**, fake ports: `loads_launcher_state`, `imports_and_refreshes_library`, `reimport_keeps_single_mod`, `remove_mod_refreshes_profiles`, `saves_profile_mod_order`, `sets_active_profile`, `handles_session_events`, `operation_error_is_exposed` and `listeners_are_released_on_dispose`.
- **Frontend/ui**, Vitest + Testing Library with fake application services: `settings_accepts_fake_unreal_directory`, `selects_detected_game`, `library_shows_mod_metadata`, `imports_from_dialog_and_drop`, `cancelled_dialog_does_not_import`, `creates_renames_and_deletes_profile`, `vanilla_delete_is_unavailable`, `profile_order_can_be_changed_with_keyboard`, `play_is_disabled_without_game_or_during_session`, `missing_app_id_error_is_visible`, `session_status_uses_cleaned_count` and `never_started_status_is_visible`.
- **Frontend/infrastructure**, SDKs mocked only at the boundary: `invoke_uses_expected_commands_and_argument_names`, covering all the commands of issue 08; `session_and_drop_adapters_unsubscribe` and `dialog_filters_supported_mod_extensions`.
- The UI test verifies that the use case refuses Play without an App ID; the proof that `~mods` remains untouched lives in the Rust application test of issue 08, without accessing disk from inside the React test.

## Definition of Done

- [ ] Red → Green → Refactor evidenced for hooks/use cases and components; tests precede the code.
- [ ] All Gherkin scenarios mapped to describe/it in Vitest; backend cucumber runners remain green.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` and Vitest scenarios green.
- [ ] Gate 6: boundaries reject ui → infrastructure; Tauri SDK only in adapters/bootstrap; Cargo dependencies preserved.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria proven, `pnpm tauri dev` smoke test on Windows with a fake folder and keyboard/focus verification; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved and Codex review applied before the PR.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
