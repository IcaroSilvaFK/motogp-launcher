---
title: "Persist JSON data and handle errors without loss"
labels: [feature, domain, application, infrastructure, interface]
depends_on: ["01"]
branch: feat/2-persist-json-data
---

## Context

Adapts Task 2. Configuration, library, profiles and manifest need persistence that supports Windows paths with spaces and accents and never replaces corrupted data with silent defaults. The plan's structures with serde derives and I/O errors cannot migrate wholesale into domain/application.

## Scope (per layer)

- **Domain:** define `Config { game_dir, active_profile }` as a pure model with optional values and a default, without serde or external library errors. Use standard path types where needed, without touching the disk.
- **Application:** introduce the configuration port and the necessary application errors. They express failures without depending on `std::io::Error`, `serde_json`, ZIP or Tauri. Use cases depend on typed ports, not on a concrete JSON store.
- **Infrastructure:** implement `DataDir` and the paths `config.json`, `profiles.json`, `library.json`, `deployed.json` and `library/`. Implement `load_json` and `save_json` and the serde DTOs; convert DTOs to pure models. A missing file returns the default, an existing invalid JSON returns an error. Save to `<path>.tmp`, create parents and replace the destination atomically on Windows, including when the file already exists; a failure must not destroy the previous version. Use pretty-print and clean up temporaries after success.
- **Interface:** reserve the resolution of Tauri's `app_data_dir` for the bootstrap of issue 08. Map application/adapter errors to a serializable error at the Tauri boundary. The old `AppError` that wraps I/O/JSON/ZIP stays out of the core; do not impose its representation on the ports.
- Reuse the storage adapter in the next issues without anticipating the library/profile/deploy entities.

## Out of scope

Profile rules, import and deploy. Schema migration and remote sync. Never treat a zero-byte file as if it did not exist: only absence is the default; a valid `[]` may represent an empty collection in issue 04.

## Acceptance criteria (Gherkin)

File: `crates/infrastructure/tests/features/persistence.feature`, run with `cucumber` and temporary directories. Application scenarios use a fake repository, with no fs/Tauri/network.

```gherkin
Feature: Persist local data safely
  Scenario: Read a missing configuration
    Given config.json does not exist yet
    When I load the configuration
    Then I get the default configuration

  Scenario: Save and read in a path with spaces and accents
    Given a directory named "Usuário Padrão"
    And a configuration with game_dir at "Program Files (x86)" and active_profile "p1"
    When I save and read config.json
    Then the values come back unchanged
    And the parent directories are created
    And no config.json.tmp file is left behind

  Scenario: Refuse corrupted JSON
    Given an existing file containing "{ not json"
    When I try to load its data
    Then I get a read error
    And the original file remains intact
    And no default is saved over it

  Scenario: Safely replace existing data on Windows
    Given config.json contains a valid configuration
    When I save a second configuration
    Then the next read returns the complete second configuration
    And there is no pending temporary file

  Scenario: Preserve the previous version when the write fails
    Given a valid file already saved
    When an I/O failure prevents the atomic replacement
    Then the operation returns an error
    And the previous file remains intact and readable
```

## TDD tests (write first)

Preserve the three plan names in **crates/infrastructure**, in the store module; they are adapter tests with `tempfile`, not domain tests:

- `missing_file_returns_default`: absence results in `Config::default()`.
- `roundtrip_in_path_with_accents`: full roundtrip and absence of `.tmp` in the accented path.
- `corrupted_json_is_an_error`: error without overwriting the original bytes.
- Add `save_replaces_existing_file_atomically` and `failed_save_preserves_previous_file` to verify the contract on Windows.
- **Application:** `config_repository_error_is_propagated`, with a fake of the port; **domain:** `Config` defaults tested without I/O.

## Definition of Done

- [ ] Red → Green → Refactor recorded; tests before code and original names preserved.
- [ ] Scenarios implemented in `.feature`/cucumber runner and traced to the tests; fixtures only in temporary directories.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test -p motogp-infrastructure --test features`, other runners (`cargo test --workspace --test features`) and Vitest scenarios green.
- [ ] Gate 6: domain with no external production deps; application depends only on domain; no rule imports fs, serde or Tauri; frontend boundaries green.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` demonstrate ≥90% domain/application per layer/crate and ≥80% overall per stack, per issue 01.
- [ ] Gate 8: all criteria proven; **Rust core** and **Frontend** checks green in CI.
- [ ] QA Haiku approved before the PR; persistence failures do not cause silent loss.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
