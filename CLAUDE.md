# motogp-launcher

## Language

**File names and file contents are always in English**: code, identifiers, comments, test names, Gherkin `.feature` files (Feature/Scenario/Given/When/Then), commit messages, branch names, docs, issues/PRs and error messages. Only the chat with the user stays in Portuguese.

Exception: **test data may be non-ASCII on purpose.** Fixtures like the Windows folder `Usuário Padrão` exist to test accented paths, which real users have. Keep them, with a short English comment saying why. QA must not reject them.

## Memory and preferences

Always save memories, decisions and preferences **locally in the project** (in this `CLAUDE.md` or in `docs/`), never only in the user's global memory.

## Development flow (multi-agent)

Everything runs inside Claude; Codex is no longer part of the flow (decided 2026-11-19). All development work follows this pipeline of roles:

1. **PO (the chat)**: point of contact with the user. Turns requests into demands (issues/cards) and hands them to the Coordinator. At the end, commits on the branch, opens and merges the PR and closes the issue/card.
2. **Coordinator (1 per issue)**: receives the demand, elaborates it into a detailed task (context, scope, Gherkin acceptance criteria, files involved) and calls the developer agents. Receives QA failures and sends them back as new dev tasks.
3. **Developer agents (N)**: created for one task, implement it with TDD in the issue's worktree, hand the result to QA and finish (one dev per task; a fix after a QA failure is a new dev). They do not commit. **Model: Sonnet** (`model: "sonnet"`).
   - **UI specialist** (decided 2026-11-23): visual redesigns, layout, design tokens and screen usability in `src/ui` go to the `ui-designer` agent ([.claude/agents/ui-designer.md](.claude/agents/ui-designer.md), **model: Opus**) instead of a Sonnet dev. Same rules as any dev (TDD, no commits, hands off to QA). Reference screenshots from other products are inspiration only: reuse patterns, never their brand. If the agent type is not loaded in the session, spawn a general-purpose agent with `model: "opus"` and the file's body as its instructions.
4. **QA agents (N)**: run the gates and write/complete the BDD scenarios. Problem found → report back to the Coordinator. No problem → notify the PO. **Model: Haiku** (`model: "haiku"`).

### Task size decides the process (decided 2026-11-26)

The full pipeline is slow and costly, so it is reserved for complex work. The PO classifies every demand before dispatching it:

| Size | Examples | Process |
|------|----------|---------|
| **Trivial** | copy or translation tweak, doc or CLAUDE.md edit, config value, catalog/ads JSON entry, one-line fix | The PO does it directly on its own branch, runs the narrow check (the touched crate's tests or `corepack pnpm test`), opens the PR and merges on green CI. No issue needed (the PR describes it), no Coordinator, no QA agent. |
| **Simple** | a bug fix or small feature inside one layer or crate, a UI tweak on one screen, no new port or IPC contract | Issue with a short task and Gherkin written by the PO. **One dev** (Sonnet; `ui-designer` for visual work) implements with TDD and runs the gates for what it touched (fmt, lint, types, the touched crate's or frontend tests). No Coordinator, no separate QA agent, no llvm-cov: CI is the full gate. A CI failure goes back to a new dev. |
| **Complex** | spans layers or crates, adds a port, IPC command or contract, touches the session/game state, or needs a design decision | Full pipeline below: Coordinator elaborates in detail (context, scope, contracts, Gherkin, files), devs, QA running the gates on the impacted parts (see "Incremental testing only"). |

When in doubt between two sizes, pick the smaller one; CI still guards the merge. If a trivial or simple task turns out bigger, promote it.

Rule for complex tasks: the chat does not implement directly. Demand (PO) → Coordinator elaborates and dispatches devs (Sonnet) → dev delivers to QA (Haiku) → QA fails: back to Coordinator / QA passes: PO → PO commits, opens the PR, merges and closes the issue → PO reports to the user.

If a subagent cannot spawn other agents, the PO spawns the devs and QA itself, using exactly the task the Coordinator wrote, and routes QA reports back to the Coordinator (via `SendMessage`).

## Git / issues flow (complex tasks; simple tasks skip the Coordinator and QA steps, trivial ones also skip the issue)

1. **PO publishes the issue** on the remote repository (`gh issue create`) with the demand.
2. **Coordinator elaborates** the task with all the details (context, scope, Gherkin acceptance criteria) and adds them to the issue.
3. **A branch is opened** from `dev`, linked to the issue, in its own worktree (e.g. `feat/<issue-no>-<slug>` in `.worktrees/<issue-no>`).
4. **Dev implements the task** in that worktree and hands it to QA.
5. **QA validates** (all gates + scenarios). Failures go back to the Coordinator, who dispatches a new dev; this loops until QA passes and notifies the PO.
6. **Integration check (PO, before every push):** QA approved the branch against the `dev` it started from, so it may not compose with what merged since. The PO commits, rebases onto the current `origin/dev` and, if `dev` moved since the branch was cut or last checked, runs the cheap integration gate in the worktree: `cargo check --workspace --all-targets` (Rust touched on either side), `cargo test -p <crate>` for every crate both sides touched, and `corepack pnpm tsc --noEmit` + `corepack pnpm test` (frontend touched on either side). Any failure goes back to the Coordinator as a normal QA failure. Typical breaks: a new enum variant hitting an exhaustive `match` written by another issue, a renamed port, a new IPC error code missing from a mapping, two BDD runner lists edited at once.
7. Only then: **the PO pushes and opens the PR to `dev`** referencing the issue (`Closes #<no>`).
8. **The PO merges on its own** (the user granted blanket merge permission on 2026-11-19): once QA approved and CI is green, merge the PR (`gh pr merge --merge --delete-branch`) without asking. `Closes #N` does not auto-close issues on merges into `dev` (the default branch is `main`), so close the issue/card manually right after the merge.

**Tooling notes:**
- **Node:** the system default is 18. Use Node 22 (`/c/Users/whyy/AppData/Local/nvm/v22.18.0` first on `PATH`) and `corepack pnpm`.
- **gh** credentials live in the Windows keyring, so `gh` and git network commands must run outside the sandbox.
- **Coverage inside worktrees:** fixed in #19; the local gate now measures the checkout it runs in.

### Machine guardrails (mandatory: the user's PC must stay usable, e.g. for Teams calls)

Parallelism applies to **agents**, never to heavy local processes. The limits below win over "parallelize by default".

- **Rust builds: one at a time, 2 jobs.** `~/.cargo/config.toml` sets one shared `target-dir` on C: (`C:/Users/whyy/.cache/motogp-target/shared`), `jobs = 2` and `RUST_TEST_THREADS = 2`. Cargo locks the shared dir, so concurrent builds queue ("Blocking waiting for file lock" = wait). **Never set `CARGO_TARGET_DIR`**: run `unset CARGO_TARGET_DIR` before cargo. A stale-rlib doctest error (`unresolved import motogp_domain::...`) is fixed with `cargo clean -p motogp-domain -p motogp-application`.
- **Process watchdog:** `C:\Users\whyy\.claude\motogp-guard.ps1` runs in the background during agent work. Every 3 s it gives cargo, rustc, linker, llvm-cov, project Node and rust-analyzer processes **idle** priority, and pins them to logical cores 0–3, leaving 4–15 free. Start it at the beginning of every session (`& "C:\Users\whyy\.claude\motogp-guard.ps1"` in a background PowerShell).
- **Incremental testing only (decided 2026-11-26):** locally, devs, QA and the PO run tests **only for the impacted parts**, never the whole workspace or the whole frontend suite:
  - Rust: `cargo test -p <crate>` for each touched crate. Add a dependent crate only when a public type or port it uses changed.
  - Frontend: `corepack pnpm vitest related <changed files> --run` (tests that import the changed files) plus the touched `.feature` test. `tsc --noEmit` and lint still run, since they are fast and catch cross-file breaks.
  - fmt and clippy only on the touched crates (`cargo clippy -p <crate>`).
  - The full suite runs **only in CI**, which stays the merge gate. A CI failure in an untouched area goes back as a normal fix task.
- **Coverage (`cargo llvm-cov`, `pnpm test --coverage`) is not run locally**, not even by QA: CI measures it. Run it locally only to investigate a CI coverage failure, and only for the affected crate.
- **Stop stale work:** when an issue's code is about to change (for example after a QA failure), stop its running QA. Remove a merged issue's worktree right away.
- **Prefer CI over local:** PR validation, rebases with no code changes and installer builds rely on GitHub Actions, not local builds.

**One branch per unit of work, always.** Every task, fix, translation or doc change gets its own branch from `dev` and its own PR; never mix unrelated changes in the same branch. Parallel work uses separate `git worktree`s inside the repo, under `.worktrees/<issue>` (git-ignored), so branches don't step on each other.

**Parallelize by default; never run issues one by one when they are independent.**
- **Hard cap: at most 10 agents working at the same time** (decided by the user on 2026-11-23). Coordinators, devs, UI specialists and QA all count. The PO tracks the count. Before spawning, it checks the cap. When the cap is reached, new issues wait in a queue and start as running agents finish. Coordinators run one child agent at a time. This cap wins over the rule below.
- At any moment, every issue whose dependencies are satisfied must be in progress (within the cap), each one with its own Coordinator/dev/QA agents in its own worktree (`.worktrees/<issue>`).
- Do not wait for a PR to be merged to start dependent work: stack the new branch on the unmerged parent branch, then rebase it onto `dev` as soon as the parent merges.
- **After every merge into `dev`**, the PO tells the Coordinators of the in-flight issues that touch the same crates or frontend layers. They rebase at the next checkpoint (between dev tasks, never mid-task, setting uncommitted work aside in a local WIP commit that the PO rewrites later) and rerun the integration gate, so a break surfaces while the issue is still in development and not only in the PR's CI. GitHub already enforces "branch up to date + green CI" on `dev`, `qas` and `main` (strict status checks); this step only moves the detection earlier.
- QA, translation/docs and CI waits run concurrently with development; the orchestrator never idles waiting on a single agent.
- Only serialize what truly conflicts: the same files, or a dependency whose contract (ports/types) doesn't exist yet.

### Standard branches

| Branch | Environment | Receives from |
|--------|-------------|---------------|
| `dev` | Development (integration) | PRs from task branches (`feat/…`, `fix/…`) |
| `qas` | Staging / QA | Promotion PR `dev → qas` |
| `main` | Production (PRD) | Promotion PR `qas → main` |

- Nobody commits directly to `dev`, `qas` or `main`: everything goes in via PR. This is **enforced on GitHub** by branch protection, applying to admins too: PR required (0 approvals), `Rust core` and `Frontend` checks required, no force-push and no branch deletion.
- The pipeline produces the **Windows installer** (NSIS setup `.exe` and `.msi`) after all gates pass. On `dev`/`qas` it stays as a pipeline artifact (30 days). On `main` it becomes a **GitHub Release** `v<version>` (version taken from `tauri.conf.json`), together with the **auto-update** `latest.json`: the installed app updates itself from the front end. Setup in [docs/auto-update.md](docs/auto-update.md).
- Promotion only moves forward (`dev → qas → main`). A hotfix starts from `main` and comes back via PR to `main`, `qas` and `dev`.

## Validation workflow: TDD + BDD + Clean Architecture (mandatory)

Details in [docs/validation-workflow.md](docs/validation-workflow.md). Summary:

- **BDD:** every issue has acceptance criteria in Gherkin, and each scenario becomes a test.
- **TDD:** Red → Green → Refactor; test always before code. Coverage ≥ 90% in domain/application and ≥ 80% overall.
- **Clean Architecture:** dependencies only point inward (`interface → infrastructure → application → domain`). Rust in separate crates (`domain`, `application`, `infrastructure`, `src-tauri`). Frontend in `domain/application/infrastructure/ui`, with boundaries checked in lint.
- **Gates:** locally, QA (Haiku) runs fmt, lint, types, tests, BDD and architecture **on the impacted parts only**. The CI in `.github/workflows/validacao.yml` runs every gate on everything, including coverage, and is the merge gate.
