---
title: "Import mods into the local library"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/3-import-mod-library
---

## Context

Adapts Task 3. Import `.pak` files with `.utoc/.ucas` siblings, or nested ZIPs, into `library/<modId>/`. The library is independent of the game folder and of the session.

## Scope (per layer)

- **Domain:** define the pure `ModEntry { id, name, files, size_bytes }` and the `group_pak_files` rule. Group by base name, recognize extensions case-insensitively, ignore unrelated extensions and require a `.pak` in each group. The display name strips only the trailing `_P`; file names keep the suffix.
- **Application:** implement list, import and remove using the `ModRepository` port, file read/copy, archive read and content hash ports, introduced as needed. Deduplicate by the 12 lowercase hex ID of the `.pak` SHA-256. Importing `.pak/.utoc/.ucas` considers only siblings with the same base name; importing a ZIP returns all valid groups. Return the existing entry on reimport. Test decisions with fakes, with no real fs or ZIP in the core.
- **Infrastructure:** implement the fs adapters, the JSON index with the store from issue 02, streaming SHA-256 and ZIP. Extract to a temporary location inside the library, flatten subfolders and ignore entries without a safe `enclosed_name()`. Do not write outside the library; reject name collisions when flattening subfolders, without overwriting files. A ZIP with no `.pak` returns an error; an unsupported extension returns an error. Record only completed imports and clean up temporaries.
- **Removal:** delete the folder of the requested ID and its index entry with error handling. Validate IDs/paths at the file boundary. Coordination with profile references will be done in issue 08, using the operation from issue 04.

## Out of scope

RAR/7z, mod download, content editing, UI, copying to `~mods` and app updating.

## Acceptance criteria (Gherkin)

Files: `crates/domain/tests/features/mod-grouping.feature` for pure rules, `crates/application/tests/features/library.feature` with fakes and `crates/infrastructure/tests/features/file-import.feature` with fs/ZIP fixtures; `cucumber` runners.

```gherkin
Feature: Organize imported mods in the library
  Scenario: Import a loose pak
    Given a file "A.pak"
    When I import it
    Then I get an entry with the file "A.pak"
    And its ID has 12 lowercase hexadecimal characters derived from the pak SHA-256
    And library.json and library/<id>/A.pak contain the imported mod

  Scenario: Pick up only the siblings of the chosen mod
    Given "A_P.pak", "A_P.utoc", "A_P.ucas" and "B.pak" in the same folder
    When I import "A_P.pak"
    Then the mod has name "A" and the three A_P files
    And B is not imported

  Scenario: Find mods in subfolders of a ZIP
    Given a ZIP containing "x/y/A.pak" and "B.pak"
    When I import it
    Then I get two entries with flattened file names

  Scenario: Reject a ZIP without a pak
    Given a ZIP containing only "readme.txt"
    When I import it
    Then I get an error containing "no .pak"

  Scenario: Reject a group without a pak
    Given a group containing only "A.utoc"
    When I validate the grouping
    Then I get an error containing "has no .pak file"

  Scenario: Reimport without duplicating
    Given a pak was already imported
    When I import the same content again
    Then I get the existing entry
    And the library still has a single entry for that ID

  Scenario: Remove a mod
    Given an imported mod
    When I remove its ID
    Then its folder and index entry no longer exist

  Scenario: Ignore directory traversal in the ZIP
    Given a ZIP containing "../evil.pak" and "ok.pak"
    When I import it
    Then only "ok.pak" enters the library
    And no file is written outside the controlled directory

  Scenario: Reject a collision when flattening subfolders
    Given a ZIP containing "x/A.pak" and "y/A.pak" with different contents
    When I import it
    Then I get a collision error
    And no existing library file is overwritten

  Scenario: Reject an unsupported format
    Given a file "mod.rar"
    When I try to import it
    Then I get an error containing "unsupported format"
```

## TDD tests (write first)

Keep the eight plan names. Separate unit policies from adapter proofs:

- **Application**, use cases with fakes: `reimport_same_pak_does_not_duplicate`.
- **Domain**, pure grouping: `utoc_without_pak_errors`; add `group_accepts_case_insensitive_extensions` and `group_ignores_unrelated_extensions`.
- **Infrastructure**, integration with temporary fs/ZIP: `import_single_pak_copies_into_library`, `import_pak_picks_up_utoc_ucas_siblings`, `import_zip_with_nested_folders_finds_all_paks`, `import_zip_without_pak_errors`, `remove_deletes_dir_and_index_entry`, `zip_entry_with_parent_traversal_is_ignored`. Exercise the use cases with real adapters, without moving I/O into application.
- **Application:** add `unsupported_extension_is_rejected` and `failed_import_does_not_publish_index_entry` with fakes.
- **Infrastructure:** add `flattened_zip_name_collision_is_rejected` and `mod_id_matches_first_twelve_sha256_hex` with known expected content and digest.

## Definition of Done

- [ ] Red → Green → Refactor evidenced; eight original names preserved and new behaviors covered before the code.
- [ ] Each scenario has a `.feature` and a cucumber test in the indicated crate; fakes in the core and fs/ZIP only in infrastructure.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features`, including the three crates, and Vitest scenarios green.
- [ ] Gate 6: Cargo deps inward and boundaries green; domain without serde/hash/ZIP/fs, application depends only on domain in production.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` with ≥90% domain/application per layer/crate and ≥80% overall per stack.
- [ ] Gate 8: all criteria traced; **Rust core** and **Frontend** green in CI.
- [ ] QA Haiku approved and Codex review applied; malicious entries do not escape the library.
- [ ] Branch from `dev`; PR to `dev` with a real `Closes #<no>`; no direct commit to `dev/qas/main`.
