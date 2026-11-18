---
title: "Manage profiles and mod order"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/4-manage-ordered-profiles
---

## Context

Adapts Task 4 and anticipates the Vanilla profile protection required by the original Interface. The order of `mod_ids` is the load order; the backend must preserve it without depending on React components.

## Scope (per layer)

- **Domain:** define `Profile { id, name, mod_ids }`, a non-empty name after trim and protection against deleting the `vanilla` ID. Preserve the given order of mods. Define missing-profile/empty-name errors as pure types.
- **Application:** implement list, create, rename, delete, find, set mods and remove a mod from all profiles through `ProfileRepository`. Introduce an ID/time source that tests can replace; keep new IDs in the `p<unix_millis>` format without collisions when creations happen close together. Do not use the real clock in core tests.
- **Initialization:** when the file is missing or holds a valid empty JSON collection, create and persist `Profile { id: "vanilla", name: "Vanilla", mod_ids: [] }`. A zero-byte file/invalid JSON remains an error from issue 02. List Vanilla before creating the first profile so it is not lost.
- **Infrastructure:** implement the JSON repository in `profiles.json`, serde DTOs and the real ID source; reuse the persistence and errors from issue 02.
- Return "empty profile name" and "profile not found" in the respective cases. The removal of references will be consumed by issue 08 after removing a mod from the library.

## Out of scope

Profile UI, deploy, sync between machines and validating mod existence against the library. Resolution of missing IDs happens in issue 08; this issue can proceed in parallel with the library and detection.

## Acceptance criteria (Gherkin)

Files: `crates/application/tests/features/profiles.feature` with fakes and `crates/infrastructure/tests/features/profile-persistence.feature` for the roundtrip; `cucumber` runners.

```gherkin
Feature: Maintain ordered profiles
  Scenario: Initialize missing profiles or an empty collection
    Given there is no profiles file or it contains a valid empty JSON collection
    When I list the profiles
    Then I get only Vanilla with ID "vanilla" and no mods
    And that profile is persisted

  Scenario: Create and list a profile
    Given Vanilla already exists
    When I create "Pinturas 2025"
    Then the list contains two profiles
    And the new profile has name "Pinturas 2025" and its own ID

  Scenario: Preserve the mod order
    Given an existing profile
    When I set its mods to "b", "a", "c"
    Then reading returns exactly that order

  Scenario: Delete a regular profile
    Given a profile other than Vanilla
    When I delete it
    Then it no longer appears in the persisted list

  Scenario: Remove references in all profiles
    Given several profiles contain the mod "a"
    When I remove the references to the mod "a"
    Then no profile contains "a"
    And the order of the other mods is preserved

  Scenario: Reject a blank name
    Given a name made only of spaces
    When I try to create or rename a profile with that name
    Then I get the error "empty profile name"
    And the previous data is preserved

  Scenario: Protect Vanilla in the backend
    Given the profile with ID "vanilla"
    When I try to delete it through the use case
    Then the deletion is refused
    And Vanilla remains saved

  Scenario: Rename a profile and handle an unknown ID
    Given an existing profile
    When I rename it to "Corrida"
    Then the updated name is persisted
    When I look up an unknown ID
    Then I get the error "profile not found"
```

## TDD tests (write first)

Preserve the six plan names in **crates/application**, use cases with a fake repository/ID source:

- `empty_store_seeds_vanilla`.
- `create_then_list`.
- `set_mods_preserves_order`.
- `delete_removes_profile`.
- `remove_mod_everywhere_strips_from_all_profiles`.
- `blank_name_is_rejected`.
- **Domain/application:** add `vanilla_cannot_be_deleted`, `rename_persists_new_name`, `unknown_profile_returns_error` and `profile_ids_do_not_collide` for the additional rules.
- **Infrastructure:** add `profiles_roundtrip_preserves_order` and `invalid_profiles_json_is_not_reseeded`, with `tempfile`. Do not move these I/O tests into the domain.

## Definition of Done

- [ ] Red → Green → Refactor demonstrated; six original names kept and Vanilla protection tested in the backend.
- [ ] Scenarios in `.feature` run with cucumber and mapped to the matching tests.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` runners and Vitest scenarios green.
- [ ] Gate 6: Cargo dependencies inward and boundaries green; no real JSON/clock reads in application/domain tests.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: criteria proven; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved and Codex review applied before the PR.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
