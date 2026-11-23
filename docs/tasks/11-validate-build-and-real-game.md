---
title: "Validate the Windows build and the real session"
labels: [chore, interface, frontend, ci]
depends_on: ["10"]
branch: feat/11-validate-build-and-real-game
---

## Context

Adapts the original Task 10, now after auto-update. Validate the integrated product on native Windows 10/11: lightweight executable, installers, mods in the game, preservation of external files, crash recovery and signed update. Tests with fakes do not replace evidence from a real game and install.

## Scope (per layer)

- **Interface/build:** run `pnpm tauri build` and validate the release executable, the NSIS setup `.exe` and the `.msi`. Since there is now a Cargo workspace at the root, the default destination is `target/release/` and `target/release/bundle/`, and not necessarily `src-tauri/target/`; respect `CARGO_TARGET_DIR` if configured. Prove the effective path of the artifacts and measure the release executable **<15 MB**.
- **CI:** confirm **Rust core** and **Frontend** before packaging, `dev/qas` artifacts retained for 30 days and the `v<version>` release/`latest.json` on `main`. Check the installer sizes separately from the executable and record the composition, including any WebView2 bootstrapper/runtime. The plan provides an objective limit for the executable; the design's lightweight-installer goal must also be evaluated and any excess justified, without omitting the size.
- **Integrated application/infrastructure:** detect a real MotoGP with the manifest's App ID, import a valid mod, choose a profile and play. Check the order/prefixes of the files, the change in the game and the cleanup after two absences in the 2 s poll (approximately 4 s, with the scheduling tolerance recorded).
- **Safety:** keep a control manual `.pak` in `~mods` during the session; compare content before/after. Test a crash by killing the launcher, close the game and reopen the launcher; the recovery removes only its own files. Do not end the user's game without agreeing on the test session; use a validation install/environment.
- **Integrated frontend/updater:** validate an update from an installed version to a later signed release, showing notes/progress and restarting on the new version. Confirm the absence of a banner when current and the preservation of library, profiles, configuration and the manual file. Use controlled test releases/host, without publishing artificial versions to production just to test.
- **Evidence:** record Windows/game/app versions, paths, sizes, gate outputs and the result per scenario in `docs/validation/` when implementing this issue, with no personal data/keys. The absence of the game, release access or a real key is a pending item, not a tacit approval.

## Out of scope

New features, Linux/Proton, copying Swofty code, engine injection and refactors not needed to fix the failures found. Do not declare the real test approved based only on mocks or a compiled build.

## Acceptance criteria (Gherkin)

File: `src-tauri/tests/features/windows-release.feature`, `cucumber` runner. Automate the inspection of artifacts and contracts in fixtures. Identify the real game/install scenarios with the tags `@windows @real_game` / `@real_updater` and run them explicitly in the Windows validation environment with evidence collection; the regular CI keeps running the deterministic suite without requiring a licensed game. The real scenarios are mandatory tests to complete this issue, even though outside the regular CI runner. UI reflections also have Vitest scenarios per issues 09/10.

```gherkin
Feature: Deliver a validated Windows release
  Scenario: Produce the executable and installers
    Given the complete workspace with approved gates
    When I run pnpm tauri build on Windows
    Then I find the release executable and the NSIS and MSI installers in the effective target
    And the executable is under 15 MB
    And the installer sizes are recorded

  @windows @real_game
  Scenario: Play with a real mod and clean up at the end
    Given an installed Steam MotoGP detected with an App ID
    And a valid mod imported and selected in a profile
    When I click Play
    Then the game shows the mod
    And the deployed files have ordered prefixes starting with "000_"
    When I close the game
    Then within approximately 4 seconds of polls the interface announces the session ended
    And only the files deployed by the launcher are removed

  @windows @real_game
  Scenario: Preserve a manual pak during the session
    Given a manual pak in ~mods with content recorded before the test
    When I run and end a session with launcher mods
    Then the manual pak keeps the same content

  @windows @real_game
  Scenario: Recover after a forced launcher shutdown
    Given a session with deployed mods and a control manual pak
    When I end the launcher through Task Manager
    And I close the game and reopen the launcher
    Then the files of the interrupted session are removed by the recovery
    And the manual pak remains intact
    And a new session can be started

  @windows @real_updater
  Scenario: Update an install preserving data
    Given an installed version with library, profiles and configuration filled in
    And a later signed version on the controlled feed
    When I start the launcher and click Update with no active session
    Then I see notes and progress
    And after installing the launcher restarts on the later version
    And library, profiles, configuration and manual files remain intact
    When I check the same feed again
    Then I see no update banner

  Scenario: Respect the per-environment distribution
    Given the reports of the environment-branch pipelines
    When I check their artifacts and publication conditions
    Then the installers were only produced after the green gates
    And dev and qas retain artifacts for 30 days
    And only main publishes the release of the configured version with latest.json containing the package signatures
```

## TDD tests (write first)

The plan did not name tests for this stage. Before adding validation logic or fixing failures:

- **Interface/build**, assertive artifact checks: `release_executable_is_below_15_mb`, `windows_bundles_include_nsis_and_msi` and `workspace_artifact_directory_is_resolved`. Use fixtures for the failure path and real artifacts for the final proof.
- **CI**, configuration contract: `installer_build_depends_on_validation`, `non_production_artifacts_have_30_day_retention` and `production_release_manifest_contains_artifact_signatures`.
- **Application**, confirm the already existing regressions `clean_removes_only_deployed_files`, `deploy_cleans_previous_session_first`, `deploy_refuses_to_overwrite_foreign_file`, `play_without_app_id_does_not_deploy` and `startup_recovers_previous_deployment`, in the layers indicated in the source issues; do not duplicate tests just to raise the count.
- **Frontend/application**, confirm `restart_only_after_successful_install` and `active_or_unclean_session_blocks_install` from issue 10.
- Any bug found first requires a Red regression test in the responsible layer, then the minimal fix and Refactor. Real game/update validations are acceptance tests, not unit tests; run the tagged scenarios and attach the evidence.

## Definition of Done

- [ ] Red → Green → Refactor demonstrated for changes/fixes; no production fix without a matching regression.
- [ ] `.feature` run with cucumber; all real scenarios completed and evidenced on Windows, without confusing exclusion from CI with a waiver of validation.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` runners, Vitest scenarios and explicit execution of the real scenarios green.
- [ ] Gate 6: Cargo deps and frontend boundaries green; build changes introduce no rules into Tauri/UI.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria traced and build/game/crash/external-file/updater evidence recorded; **Rust core** and **Frontend** green in CI.
- [ ] Executable <15 MB; NSIS/MSI produced; installer sizes/limitations documented and evaluated by QA.
- [ ] QA Haiku approved; a pending item for hardware, game, feed or signature prevents declaring this issue complete.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; promotion only `dev → qas → main`, no direct commit to those branches.
