---
title: "Update the app from the UI"
labels: [feature, frontend, infrastructure, interface, ci]
depends_on: ["09"]
branch: feat/10-in-app-auto-update
---

## Context

Extra issue required by `docs/auto-update.md`, placed after the Interface and before the release validation. The documentation and the current request now include auto-update, overriding the old design's non-goal. The app checks production releases on startup and only installs after a user action.

## Scope (per layer)

- **Frontend/domain:** define only the pure data needed for version, notes and progress, without updater SDK types.
- **Frontend/application:** create `ports/UpdateService.ts` with `check()`, `downloadAndInstall(onProgress)` and `restart()`, and `useCases/checkForUpdate.ts`. Coordinate the check on startup, download, install and restart through the port. Use the updater's version semantics, without comparing strings lexicographically. The absence of an update shows no banner; errors are recoverable and do not block the rest of the launcher.
- **Frontend/infrastructure:** implement `tauri/TauriUpdateService.ts` with `@tauri-apps/plugin-updater` and `@tauri-apps/plugin-process`, preserving the validated update object returned by the check. Propagate start/progress/end, network, signature and install failures. Progress without a known total size may be indeterminate.
- **Frontend/ui:** create a presentation-only `UpdateBanner.tsx`, under `src/ui`, with an accessible version, notes in a safe format, progress bar and Update button. Avoid a double install; restart only after the install completes. Disable the install while a session is starting/active or cleanup is pending, so as not to kill the watcher with mods deployed; revalidate that condition in the use case, not only in the button.
- **Interface/Tauri configuration:** register the Rust updater and process plugins, the matching JS dependencies and the capabilities `updater:default` and `process:allow-restart`. Configure `bundle.createUpdaterArtifacts = true`, the **public** key, the real endpoint `https://github.com/<owner>/<repo>/releases/latest/download/latest.json` and Windows `installMode: "passive"`. The bootstrap injects the service; Rust domain/application remain free of updater dependencies.
- **CI/release:** adapt the existing workflow, keeping the gates before the build. Use the version from `tauri.conf.json`, NSIS/MSI signed for updating and a `v<version>` release with `latest.json` on `main`. `dev/qas` produce artifacts for 30 days, without publishing a production release. Preserve the existing behavior instead of duplicating jobs.
- **External setup needed to complete this issue:** generate the key pair in a safe place outside the repository; register `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as GitHub secrets; version only the public one. Identify owner/repo and confirm access to the feed without an embedded token. If the repository is private, arrange public distribution of the artifacts/manifest or an appropriate host, without assuming anonymous access to the private release. The issue cannot be closed with a fictitious endpoint/key or with the signature disabled. Record only non-secret evidence.

## Out of scope

Updating mods or the game, silent installation without a click, a GitHub token in the app, beta channels and changing the coverage gates. Do not version the private key nor expose a secret in logs. Executing this planning does not generate keys, change secrets or publish releases.

## Acceptance criteria (Gherkin)

Source: `tests/features/frontend/auto-update.feature`; frontend scenarios in Vitest + Testing Library, especially `src/application/useCases/__tests__/checkForUpdate.test.ts` and `src/ui/__tests__/UpdateBanner.test.tsx`. Windows signature/configuration contracts: `src-tauri/tests/features/updater.feature`, `cucumber` runner with local fixtures. Automated gates use fakes/fixtures; the real update with two versions is proven in issue 11.

```gherkin
Feature: Update the installed launcher through the interface
  Scenario: Find and install a newer version
    Given the installed app is on version "1.0.0"
    And the service reports the release "1.1.0" with notes and a valid signature
    When the app starts
    Then I see "New version 1.1.0 available" and the notes
    When I click Update with no pending game session
    Then I see the download progress
    And the installation completes before the restart request
    And the app restarts on version "1.1.0"

  Scenario: Already on the latest version
    Given the installed app and the latest release are on version "1.1.0"
    When the app starts
    Then no update banner is shown
    And no download or restart is requested

  Scenario: Do not install an older version
    Given the app on version "1.10.0" and a feed with "1.9.0"
    When I check for updates
    Then the older release is not offered for installation

  Scenario: Keep the launcher usable offline
    Given the feed query fails due to the network
    When the app starts
    Then I see a recoverable notice
    And I can still use library, profiles and settings
    And no restart is requested

  Scenario: Refuse a package with an invalid signature
    Given an update whose package fails the signature verification
    When I try to update it
    Then the installation is refused and the error is visible
    And the app neither restarts nor replaces the installed version

  Scenario: Handle an installation failure
    Given the download finishes but the installation fails
    When the service returns the error
    Then the error is visible and the attempt is no longer in progress
    And no restart is requested

  Scenario: Avoid simultaneous installs
    Given an update in progress
    When I trigger Update again
    Then there is no second download nor second installation

  Scenario: Protect a game session
    Given a session starting, running or awaiting cleanup
    When an update becomes available
    Then I can see the new version
    But the installation and restart are unavailable until the session is clean

  Scenario: Publish the feed only through the production flow
    Given a push to an environment branch with approved gates
    When the pipeline produces the installers
    Then dev and qas keep NSIS and MSI for 30 days with no production release
    And main publishes the release of the configured version with latest.json and signed artifacts
    And the app configuration contains only the public key
```

## TDD tests (write first)

There are no original names in this extra issue. Write first:

- **Frontend/application**, fake `UpdateService`: `checkForUpdate_returns_new_release`, `checkForUpdate_returns_none_when_current`, `older_release_is_not_offered`, `check_failure_does_not_block_launcher`, `download_progress_is_forwarded`, `restart_only_after_successful_install`, `install_failure_does_not_restart`, `concurrent_update_is_rejected` and `active_or_unclean_session_blocks_install`.
- **Frontend/ui**, Testing Library: `shows_new_version_notes_and_update_button`, `hides_banner_when_up_to_date`, `shows_determinate_or_indeterminate_progress`, `update_error_is_accessible` and `session_state_disables_update_action`.
- **Frontend/infrastructure**, SDK mocked at the boundary: `updater_adapter_checks_downloads_installs_and_restarts`, `invalid_signature_error_is_propagated` and `version_check_uses_updater_semantics`; the real refusal of an invalid package also requires a contract test, without simulating plugin success.
- **Interface/CI:** `updater_config_contains_public_key_and_endpoint`, `updater_capabilities_are_minimal`, `unsigned_or_tampered_update_is_rejected` with a local signature fixture and `release_job_requires_successful_validation`. Check the conditions/artifacts of the existing workflow without publishing in tests.

## Definition of Done

- [ ] Red → Green → Refactor evidenced; frontend scenarios implemented as describe/it, signature/configuration covered in the cucumber runner.
- [ ] Setup of the public key, secrets and real endpoint completed with evidence free of secrets; feed access proven in the context of the installed app.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` and Vitest scenarios green, without depending on a mutable public feed.
- [ ] Gate 6: boundaries/Cargo deps green; updater/process SDK only in adapters and bootstrap, no direct import by the UI.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria traced to tests; **Rust core** and **Frontend** green; build/release conditioned on the gates.
- [ ] QA Haiku approved and Codex review applied; the integrated validation of the real update is explicitly left to issue 11, without claiming a fake proves a restart on the new version.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; promotions `dev → qas → main`, no direct commit to those branches.
