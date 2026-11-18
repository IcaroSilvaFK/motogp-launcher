---
title: "Create the scaffold and validation gates"
labels: [chore, domain, application, infrastructure, interface, frontend, ci]
depends_on: []
branch: feat/1-scaffold-and-gates
---

## Context

Adapts Task 1 of the plan. Set up Tauri 2, React + TypeScript + Vite and pnpm in `D:\www\motogp-launcher`, on native Windows with stable Rust MSVC. Preserve the existing documents, history and workflows: the old procedure of moving/deleting the project and running `git init` does not apply.

## Scope (per layer)

- **Rust domain:** create `crates/domain`, with no external production dependencies, serde, Tauri or other project crates. Reserve pure entities and rules for the next issues.
- **Rust application:** create `crates/application`, with a production dependency only on `domain`; use cases and ports/traits will be introduced in the issues that use them. Test dependencies, such as `cucumber`, go in `dev-dependencies`.
- **Rust infrastructure:** create `crates/infrastructure` for adapters; fs/JSON, ZIP/SHA-256 and Steam/process dependencies come in here when needed, never in the core.
- **Rust interface:** keep `src-tauri` as a member of the root Cargo workspace, with the `motogp_launcher_lib` library, bootstrap and future composition of the adapters. Fix the Cargo package names as `motogp-domain`, `motogp-application`, `motogp-infrastructure` and `motogp-launcher`, respectively, to make the validation commands reproducible. Configure the window as MotoGP Launcher, 1100×700, minimum 900×600, identifier `dev.motogplauncher.app`. Reserve thin commands, DTO conversion and error mapping.
- **Frontend:** create `src/domain`, `src/application`, `src/infrastructure` and `src/ui`; React stays in presentation and in application hooks when needed. Configure `eslint-plugin-boundaries`: domain imports no outer layers; application does not import infrastructure/ui; infrastructure implements ports; ui uses domain/application and does not import infrastructure. Do adapter injection in `src/main.tsx`, the composition root with an explicit exception restricted to that file. Pure types live in domain; Tauri, dialog and event SDKs live in infrastructure.
- **Tooling/CI:** configure rustfmt, Clippy, TypeScript, ESLint/boundaries, Prettier, Vitest + Testing Library and coverage. `pnpm test` must finish in non-interactive mode. Declare the package manager and generate the pnpm lockfile. Configure the Rust BDD `features` targets, with `harness = false`, `cucumber` as a dev dependency and `tests/features/` in each crate that runs scenarios; the frontend uses Vitest for the Gherkin scenarios.
- **Existing CI:** adapt `.github/workflows/validacao.yml` keeping the required checks **Rust core** and **Frontend**, PRs/pushes to `dev`, `qas`, `main` and a build after the gates. Do not accept skipped jobs as evidence of a validated scaffold. Rust core runs on Windows; the frontend may keep the existing Linux runner. Run fmt, lint, types, unit tests, BDD, Cargo dependency validation and coverage. Enforce ≥90% line coverage on **each** domain/application crate and on the respective frontend layers; enforce ≥80% overall per stack. The current overall Rust limit of 80% alone does not satisfy the 90% core rule. Do not use broad exclusions to hide untested logic.
- **Execution:** local commands run from the workspace root. Use `cargo test --workspace --test features` for the BDD runners, equivalent to the `cargo test --test features` gate with explicit selection of the whole workspace. For a single runner, use `cargo test -p motogp-application --test features`, for example. Preserve the future NSIS/MSI and release generation, without anticipating the signing configuration of issue 10.

## Out of scope

Import, profiles, Steam detection, deploy, session and updater. Do not install tools globally if the prerequisites are already available; document the missing ones. Do not replace documents or workflows with empty templates, nor run the destructive script of the old scaffold.

## Acceptance criteria (Gherkin)

Rust: `src-tauri/tests/features/scaffold.feature`, `cucumber` runner; frontend: `src/ui/__tests__/scaffold.test.tsx`, Vitest + Testing Library. The workspace/tooling checks are interface integration tests, with no I/O in the core tests. The command and CI scenarios must run the real checks; opening the window has a smoke test on Windows.

```gherkin
Feature: Start the project with boundaries and mandatory validation
  Scenario: Generate the scaffold preserving the project
    Given the project contains versioned documents and workflows
    When the scaffold is integrated into the current root
    Then the documents and the history are preserved
    And the workspace contains domain, application, infrastructure and src-tauri
    And the workspace and frontend tests finish successfully

  Scenario: Open the application on Windows
    Given the MSVC, Rust, Node, pnpm and WebView2 prerequisites are available
    When I run the application with pnpm tauri dev
    Then I see the MotoGP Launcher window with the specified dimensions
    And the frontend renders without errors

  Scenario: Reject dependencies that violate the architecture
    Given an import from ui to infrastructure in a lint fixture
    And a dependency from application to infrastructure in a Cargo fixture
    When I run the architecture validators
    Then each violation is rejected
    And the authorized composition root remains valid

  Scenario: Run all gates in CI
    Given a PR to dev with Rust and frontend code
    When the validation workflow runs
    Then Rust core and Frontend pass by running their gates
    And lint, type, BDD or coverage failures fail the validation
    And core coverage is at least 90 percent and overall 80 percent
```

## TDD tests (write first)

The plan did not name tests for this task. Write the scaffold checks first, without inventing business rules to fill coverage:

- **Architecture tooling:** `rejects_ui_importing_infrastructure` and `rejects_outward_cargo_dependencies`, with isolated fixtures that do not make the main project invalid.
- **Frontend/ui:** `renders_launcher_shell`, an accessible render smoke test in Vitest.
- **Tooling integration:** `workspace_members_are_registered` and `coverage_thresholds_reject_insufficient_coverage`, checking metadata/configuration and the failure behavior of the gates. They are not business unit tests.
- Record Red → Green → Refactor; while there is no measurable core code, document that condition without creating artificial rules or relaxing the limits for the next issues.

## Definition of Done

- [ ] Tests written before the implementation; Red → Green → Refactor evidence and `test:` commits before or together with `feat:`/`chore:`.
- [ ] All the scenarios above have matching tests; window smoke test run on Windows.
- [ ] Gate 1: `cargo fmt --all --check` and `pnpm prettier --check .` green.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` and `pnpm lint` green.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` and `pnpm test` green.
- [ ] Gate 5: `cargo test --workspace --test features` runners and Vitest scenarios green, requiring no game or network.
- [ ] Gate 6: Cargo metadata and boundaries confirm inward dependencies; only dev-dependencies may contain the core BDD tooling.
- [ ] Gate 7: `cargo llvm-cov` and `pnpm test --coverage` enforce ≥90% domain/application and ≥80% overall; initial absence of core lines made explicit, with no bypass once there is an implementation.
- [ ] Gate 8: acceptance criteria traced to tests; **Rust core** and **Frontend** green in CI, no skips.
- [ ] QA Haiku approved and Codex review applied before opening the PR.
- [ ] Branch from `dev`; PR to `dev` with `Closes #<no>` using the real number after publication. Promotions only `dev → qas → main`; no direct commits on those branches.
