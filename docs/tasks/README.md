# MotoGP Launcher issues

These 11 issues adapt the 10 tasks of the 2026-11-18 plan to the TDD, BDD and Clean Architecture flow. Issue 10 adds auto-update; the original Task 10 becomes issue 11. The files are prepared for future publication: the GitHub issue numbers equal the plan order NN (issue 1 = 01, and so on).

| NN | Title | Depends on | Branch |
| --- | --- | --- | --- |
| 01 | [Create the scaffold and validation gates](01-scaffold-and-gates.md) | — | `feat/1-scaffold-and-gates` |
| 02 | [Persist JSON data and handle errors without loss](02-persist-json-data.md) | 01 | `feat/2-persist-json-data` |
| 03 | [Import mods into the local library](03-import-mod-library.md) | 02 | `feat/3-import-mod-library` |
| 04 | [Manage profiles and mod order](04-manage-ordered-profiles.md) | 02 | `feat/4-manage-ordered-profiles` |
| 05 | [Detect MotoGP installs through Steam](05-detect-game-installs.md) | 02 | `feat/5-detect-game-installs` |
| 06 | [Deploy and clean mods with a safe manifest](06-deploy-and-clean-mods.md) | 03 | `feat/6-deploy-and-clean-mods` |
| 07 | [Launch the game and track the session](07-launch-and-track-session.md) | 05 | `feat/7-launch-and-track-session` |
| 08 | [Integrate Tauri commands and session recovery](08-tauri-commands-and-recovery.md) | 03, 04, 05, 06, 07 | `feat/8-tauri-commands-and-recovery` |
| 09 | [Build the launcher UI](09-launcher-ui.md) | 08 | `feat/9-launcher-ui` |
| 10 | [Update the app from the UI](10-in-app-auto-update.md) | 09 | `feat/10-in-app-auto-update` |
| 11 | [Validate the Windows build and the real session](11-validate-build-and-real-game.md) | 10 | `feat/11-validate-build-and-real-game` |

The dependencies in the frontmatter are local two-digit references; transitive ones are implicit. After 02, issues 03/04/05 can proceed independently; 06 requires 03 and 07 requires 05. This indicates what can be executed, without starting agents or development in this planning step. 08 integrates the five deliveries, 09 delivers the UI, 10 adds updating and 11 validates the whole.

## Sources and precedence

Read [CLAUDE.md](../../CLAUDE.md), [validation workflow](../validation-workflow.md), [auto-update](../auto-update.md), [design](../superpowers/specs/2026-11-18-motogp-launcher-design.md) and [original plan](../superpowers/plans/2026-11-18-motogp-launcher.md). The current request and the new rules replace the old monolithic organization and the auto-update non-goal. The existing workflow [.github/workflows/validacao.yml](../../.github/workflows/validacao.yml) was taken into account to avoid recreating jobs that already exist.

## Adaptation decisions

- **Layers:** Cargo workspace at the root with `crates/domain`, `crates/application`, `crates/infrastructure`, `src-tauri`; pure models in domain, ports/use cases in application, serde/fs/ZIP/Steam/processes in adapters, DTOs/composition/thin commands in Tauri. Production dependencies of application are limited to domain; `cucumber` and other test helpers are dev-dependencies, with no I/O exception in core code.
- **Frontend:** `src/domain|application|infrastructure|ui`, with `eslint-plugin-boundaries`; injection in `src/main.tsx`, composition exception restricted to that file. Pure types do not import the SDK and ui never imports infrastructure.
- **BDD:** Rust uses `tests/features/*.feature` in the responsible crate and a `features` target with `harness = false`/the cucumber crate. Run them all with `cargo test --workspace --test features`, or select with `-p motogp-domain`, `-p motogp-application`, `-p motogp-infrastructure` and `-p motogp-launcher`, names fixed in issue 01. The frontend keeps its Gherkin in `tests/features/frontend/` and runs it as Vitest + Testing Library describe/it, per the workflow; it does not use Rust cucumber for React components. Each scenario must have a traceable test, including integration when it is not a unit test.
- **TDD:** the **35 original test names** of Tasks 2–8 were preserved in issues 02–08. fs/ZIP/Steam tests with `tempfile` remain infrastructure contracts/integration; rules/use cases are tested without disk/network/Tauri using fakes. Original Tasks 1, 9 and 10 did not name unit tests; the corresponding issues propose new checks, without claiming they already exist.
- **Gates:** 01 adapts the existing CI so that Rust core and Frontend run all gates and enforce ≥90% on each domain/application and ≥80% overall per stack. The initial absence of core lines is made explicit, never replaced by a fictitious business rule or by disabling the later limits.
- **Consistency:** the App ID is validated before deploy, reconciling Task 8 with Task 9's manual requirement of not creating `~mods` when the ID is missing. Vanilla is protected in the application, beyond the UI; deleting the active profile falls back to Vanilla. A zero-byte JSON is corruption; only a missing file or a valid empty JSON collection allows seeding profiles.
- **Files and crash:** the per-file manifest write was kept, but issue 06 requires tested recovery across the interruption windows, with ownership tracking and no deletion by prefix. ZIP name collisions, overwrite in the copy fallback, a manifest with an unsafe path and partial cleanup have explicit criteria. Crash recovery happens on the next launch; there is no promise of cleanup while the launcher is closed.
- **Updater:** requires a click, a valid signature and a clean session before installing/restarting. Owner/repo, public access to the feed, public key and secrets are external prerequisites to confirm when implementing issue 10; no value was invented and no key/token is delivered in these files. Without an accessible feed there is no real-update acceptance; do not assume anonymous access to a private release. `latest.json` carries the package signatures, without requiring a separate signature of the JSON itself.
- **Build:** the workspace changes the default output to `target/release`, respecting the effective configuration. The plan's objective limit is an executable <15 MB; also record the installer sizes and evaluate the design's lightweight-installer goal. The WebView2 bootstrapper may affect the installer. Real game/update tests are mandatory in issue 11, in a controlled Windows environment, in addition to the deterministic CI suite.

## Publication and future flow

Publish each body as an issue later, apply labels and replace local references with real links/numbers when available. `NN` is the plan order and matches the GitHub issue number. The `branch: feat/<N>-<slug>` field uses the issue number without the leading zero; if the GitHub numbering ever differs, explicitly link the real issue in the branch/PR and use `Closes #<real-no>`, without assuming NN matches it.

Task branches come from `dev`. Developers follow TDD, QA Haiku runs the eight gates (failures go back to the Coordinator) and the PO opens the PR to `dev` once QA passes. Promotion happens via PR `dev → qas → main`; hotfixes follow the CLAUDE.md flow. Do not create direct commits on those branches nor promote without the Rust core and Frontend checks.

This batch contains Markdown specifications only. It does not run scaffolding, product tests, Git/gh commands, issue publication, secrets configuration or releases.
