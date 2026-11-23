---
title: "Deploy and clean mods with a safe manifest"
labels: [feature, domain, application, infrastructure]
depends_on: ["03"]
branch: feat/6-deploy-and-clean-mods
---

## Context

Adapts Task 6. The `deployed.json` manifest is the source for session recovery; user files must never be overwritten or deleted. Cleanup after a crash happens on the launcher's next launch, per the design, and will be wired into the bootstrap in issue 08.

## Scope (per layer)

- **Domain:** define `Deployed { mods_dir, files }` and `deployed_name(index, mod_id, original)`, pure. The index is the position of the mod, starting at zero, and is shared by the files of the same mod. Name: `NNN_<modId>_<originalName>`, preserving the original `_P` before the extension.
- **Application:** implement deploy and clean with filesystem and manifest-repository ports. Before a new deploy, clean the previous session; if the cleanup fails, do not proceed. An empty profile creates no folder or new files. For each file, refuse an existing destination and use exclusive creation, with no gap that allows a concurrent overwrite. Update the manifest after each deployed file, never only at the end.
- **Infrastructure:** implement hard link and a copy fallback, including across volumes. The fallback must also refuse overwriting. Implement removal of only the registered, safe paths, tolerating `NotFound`. When cleanup finishes, persist `Deployed::default()` and return the count actually removed; on partial failure, retain the still-pending items for a retry.
- **Safety and recovery:** validate simple relative names and confinement to the authorized directory, including a manifest read from disk; do not accept `..`/absolute paths. Record and test the protocol between materialization and persistence so that no deployed file is left without recoverable tracking on interruption. The old "create and only then save" sequence has a crash window and, by itself, does not prove this guarantee; do not solve it by deleting by prefix or by including a foreign file in the manifest. Use a recoverable journal/ownership record, with interruption tests at the critical points.
- Reuse the library and persistence from issues 03/02; `deploy` consumes already-ordered entries, without knowing the UI or concrete profiles.

## Out of scope

Deleting the whole `~mods`, scanning/deleting by prefix, replacing external files, watcher/processes and cleanup while the launcher is dead. Do not promise immediate cleanup after a crash without the reopening foreseen in the design.

## Acceptance criteria (Gherkin)

Files: `crates/domain/tests/features/deploy-names.feature`, `crates/application/tests/features/deploy.feature` with fakes and injected failures, `crates/infrastructure/tests/features/file-deploy.feature` with a temporary fs; `cucumber` runners.

```gherkin
Feature: Deploy only the session mods and recover their cleanup
  Scenario: Deploy in the profile order
    Given the mods B and A in that order
    When I deploy their files
    Then the destinations are "000_<idB>_B.pak" and "001_<idA>_A.pak"
    And each deployed file has a recoverable persisted record

  Scenario: Preserve the patch suffix
    Given an original file "Ducati_P.pak"
    When I compute its destination
    Then the name ends in "_Ducati_P.pak"

  Scenario: Clean only what was deployed
    Given registered deployed files and the manual file "meu_mod.pak" in ~mods
    When I clean the session
    Then only the registered files are removed
    And meu_mod.pak keeps the same bytes
    And the manifest is empty after a complete cleanup

  Scenario: Tolerate an already-missing deployed file
    Given a registered file was already removed
    When I clean the session
    Then the cleanup finishes without an error for that file
    And the count considers only files actually removed

  Scenario: Clean the previous session before redeploying
    Given a previous session with A deployed
    When I deploy only B
    Then the files of A are removed before those of B
    And only B is registered

  Scenario: Refuse a collision with a user file
    Given a manual file with the exact name of the planned destination
    When I try to deploy the mod by hard link or copy fallback
    Then I get an error containing "already exists"
    And the manual file stays intact and outside the cleanup

  Scenario: Deploy an empty profile without creating a directory
    Given there is no previous session nor a ~mods folder
    When I deploy a profile without mods
    Then ~mods remains absent

  Scenario: Recover from an interruption during deploy
    Given an interruption at a point between the record and the materialization of a file
    When I resume recovery from the manifest and the journal
    Then every file proven deployed can be cleaned
    And no foreign file is deleted

  Scenario: Retry a partially failed cleanup
    Given two registered files and a failure when removing the second
    When I clean the session
    Then I get an error
    And the second file remains tracked for recovery

  Scenario: Reject a path outside the authorized directory
    Given a tampered manifest with a path containing ".."
    When I try to clean the session
    Then the operation returns an error
    And no external file is removed
```

## TDD tests (write first)

Preserve the seven plan names:

- **Domain**, unit: `deploy_preserves_p_suffix`.
- **Application**, unit tests with a fake fs/manifest: `deploy_writes_files_in_profile_order_with_prefix`, `deploy_cleans_previous_session_first`, `deploy_empty_profile_leaves_dir_untouched`.
- **Infrastructure**, contracts with a temporary fs: `clean_removes_only_deployed_files`, `clean_tolerates_already_missing_file`, `deploy_refuses_to_overwrite_foreign_file`.
- **Application:** add `interrupted_deploy_remains_recoverable`, with interruptions between record/materialization, `failed_clean_keeps_pending_entries` and `invalid_manifest_path_is_rejected`.
- **Infrastructure:** add `copy_fallback_never_overwrites_existing_file` and `hard_link_failure_falls_back_to_copy`, using a controlled adapter failure so as not to require two volumes in CI. Also cover the successful hard link path on Windows.

## Definition of Done

- [ ] Red → Green → Refactor evidenced; seven original names preserved; critical failures tested before the implementation.
- [ ] Scenarios in `.feature`/cucumber in the correct crates; recovery proves ownership, with no name/prefix heuristic.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` and Vitest scenarios green.
- [ ] Gate 6: Cargo dependencies inward and boundaries green; rules with no I/O and adapters not deciding profile order.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria and disk/interruption failures proven; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved; foreign files never overwritten/deleted and pending items not discarded.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
