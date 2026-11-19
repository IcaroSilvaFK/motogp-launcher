---
title: "Detect MotoGP installs through Steam"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/5-detect-game-installs
---

## Context

Adapts Task 5. The install may be in any Steam library. The App ID comes from the matching manifest, with no constant for MotoGP 24/25 or any other edition. Manual selection must work even without Steam detected.

## Scope (per layer)

- **Domain:** define the pure `GameInfo { name, root, mods_dir, app_id }`, with an optional `app_id`. Keep the MotoGP selection rules, case-insensitive `installdir` matching and exclusion of the Engine folder free of fs/registry/serde.
- **Application:** implement detect installs and inspect a root using Steam location and game-structure reading ports. Do the selection and validation, without calling `winreg` or fs directly. A missing Steam returns an empty list; a root without `Content/Paks` returns "folder does not look like an Unreal game: Content/Paks not found". Inspection does not create `~mods`.
- **Infrastructure:** implement `parse_library_folders`, `parse_app_manifest`, physical discovery of `Content/Paks`, manifest reading and a Windows adapter for `HKCU\Software\Valve\Steam`, value `SteamPath`. VDF/ACF parsers belong to the Steam format adapter, even though they are pure functions. Unescape backslashes in `libraryfolders.vdf`; look for folders in `steamapps/common` with a case-insensitive MotoGP prefix. Look up the App ID in the `appmanifest_*.acf` files of `steamapps`, whose `installdir` matches the root folder. Ignore `Engine` when searching for the first immediate subfolder with `Content/Paks`.
- Derive `mods_dir` as `<root>/<Project>/Content/Paks/~mods`, without hard-coding the project name. Keep the Windows adapters conditional on the platform without bringing Windows dependencies into the core.

## Out of scope

Fixed App ID, starting Steam/the game, creating `~mods`, settings UI and Linux/Proton support.

## Acceptance criteria (Gherkin)

Files: `crates/application/tests/features/game-detection.feature` with fake ports and `crates/infrastructure/tests/features/steam.feature` with a temporary Steam tree; `cucumber` runners. The real registry is a separate Windows smoke test, not a requirement for automated tests.

```gherkin
Feature: Find the correct MotoGP install
  Scenario: Parse Steam libraries with escaped backslashes
    Given a libraryfolders.vdf with "C:\\Program Files (x86)\\Steam" and "D:\\SteamLibrary" escaped in VDF format
    When I read the libraries
    Then I get the two Windows paths with unescaped backslashes
    And the spaces are preserved

  Scenario: Read the App ID and the install directory
    Given an ACF manifest with appid "123" and installdir "MotoGP25"
    When I parse the manifest
    Then I get the ID 123 and the name "MotoGP25"

  Scenario: Inspect a valid Unreal game
    Given "common/MotoGP25/MotoGP25/Content/Paks"
    And an appmanifest in steamapps with installdir "motogp25" and appid "123"
    When I inspect the root "common/MotoGP25"
    Then mods_dir ends in "MotoGP25/Content/Paks/~mods"
    And the App ID is 123
    And the ~mods folder is not created

  Scenario: Reject a root without project Paks
    Given a root without Content/Paks in its project subfolders
    When I inspect it
    Then I get an error containing "Content/Paks not found"

  Scenario: Detect only MotoGP
    Given libraries with a valid MotoGP25 and a valid OtherGame
    When I detect the games
    Then only MotoGP25 appears

  Scenario: Ignore the Engine folder
    Given a root with Engine/Content/Paks and Project/Content/Paks
    When I look for the game packages
    Then I use Project/Content/Paks

  Scenario: Allow a manual folder without a Steam manifest
    Given a valid Unreal root without a matching manifest
    When I inspect it
    Then I get a GameInfo with no App ID
    And no mods folder is created

  Scenario: Steam not found in the registry
    Given SteamPath is not available
    When I detect installs automatically
    Then I get an empty list
    And manual inspection remains available
```

## TDD tests (write first)

Preserve the five plan names:

- **Infrastructure/Steam**, pure parsing functions: `parse_library_folders_unescapes_backslashes`, `parse_app_manifest_reads_id_and_installdir`.
- **Infrastructure**, inspection with a `tempfile` tree: `inspect_game_finds_mods_dir_and_app_id`, `inspect_game_without_paks_errors`, `detect_games_only_returns_motogp_folders`. Exercise the use cases with adapters, keeping the selection rule in the core.
- **Application**, unit tests with fakes: `missing_steam_returns_empty_list`, `manual_game_without_manifest_has_no_app_id`, `engine_folder_is_not_selected` and `manifest_match_is_case_insensitive`.
- **Infrastructure:** `inspection_does_not_create_mods_directory`, ensuring no write side effects.

## Definition of Done

- [ ] Red → Green → Refactor evidenced, keeping the five original names and fixtures independent of an installed Steam.
- [ ] All scenarios have a `.feature` and a cucumber runner; Windows registry smoke test recorded separately.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` and Vitest scenarios green.
- [ ] Gate 6: Cargo dependencies inward and boundaries green; winreg/fs/Steam parsers stay outside the core.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria proven and no fixed App ID; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved before the PR.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
