# motogp-launcher

## Language

**File names and file contents are always in English**: code, identifiers, comments, test names, Gherkin `.feature` files (Feature/Scenario/Given/When/Then), commit messages, branch names, docs, issues/PRs and error messages. Only the chat with the user stays in Portuguese.

Exception: **test data may be non-ASCII on purpose.** Fixtures like the Windows folder `Usuário Padrão` exist to test accented paths, which real users have. Keep them, with a short English comment saying why. QA must not reject them.

## Memory and preferences

Always save memories, decisions and preferences **locally in the project** (in this `CLAUDE.md` or in `docs/`), never only in the user's global memory.

## Development flow (multi-agent)

All development work follows this pipeline of roles:

1. **General specialized agent (the chat)**: point of contact with the user; understands the request, orchestrates the flow and consolidates the result.
2. **Codex**: organizes and writes up the tasks (breaks the work down into detailed tasks).
3. **PO agent (1)**: coordinates the developers: hands out the tasks and follows the delivery.
4. **Developer agents (N)**: implement the tasks, in parallel when possible. **Model: Sonnet** (`model: "sonnet"`).
5. **QA agents (N)**: test and validate what the devs delivered. **Model: Haiku** (`model: "haiku"`).
6. **Codex**: final review and improvements to what was implemented.

Rule: the chat does not implement directly. Development request → Codex generates the tasks → PO dispatches the devs (Sonnet) → QA validates (Haiku) → Codex reviews → chat reports to the user.

## Git / issues flow (mandatory for every task)

1. **Codex writes up** the task with all the details (context, scope, acceptance criteria).
2. **Publishes an issue** on the remote repository (`gh issue create`) with that detail.
3. **Dev opens a branch** from `dev`, linked to the issue, to keep the history (e.g. `feat/<issue-no>-<slug>`).
4. **Dev finishes the task** on the branch.
5. The delivery **goes through the whole review** (QA Haiku + Codex review/improvements).
6. Only then: **opens the PR to `dev`** referencing the issue (`Closes #<no>`) and **closes the issue**.
7. **The orchestrator merges on its own** (the user granted blanket merge permission on 2026-11-19): once QA approved, the Codex review is applied and CI is green, merge the PR (`gh pr merge --merge --delete-branch`) without asking. `Closes #N` does not auto-close issues on merges into `dev` (the default branch is `main`), so close the issue manually right after the merge.

**Tooling notes:**
- **Codex sandbox:** it can only write in the main checkout, not in `.worktrees/`, and it cannot run `git add`/commit or esbuild/Vitest. Use Codex for **read-only reviews** of worktree branches; a Sonnet dev applies the findings, and the orchestrator commits when needed.
- **Node:** the system default is 18. Use Node 22 (`/c/Users/whyy/AppData/Local/nvm/v22.18.0` first on `PATH`) and `corepack pnpm`.
- **gh** credentials live in the Windows keyring, so `gh` and git network commands must run outside the sandbox.
- **Coverage inside worktrees:** fixed in #19; the local gate now measures the checkout it runs in.

### Machine guardrails (mandatory: the user's PC must stay usable, e.g. for Teams calls)

Parallelism applies to **agents**, never to heavy local processes. The limits below win over "parallelize by default".

- **Rust builds: one at a time, 2 jobs.** `~/.cargo/config.toml` sets one shared `target-dir` on C: (`C:/Users/whyy/.cache/motogp-target/shared`), `jobs = 2` and `RUST_TEST_THREADS = 2`. Cargo locks the shared dir, so concurrent builds queue ("Blocking waiting for file lock" = wait). **Never set `CARGO_TARGET_DIR`**: run `unset CARGO_TARGET_DIR` before cargo. A stale-rlib doctest error (`unresolved import motogp_domain::...`) is fixed with `cargo clean -p motogp-domain -p motogp-application`.
- **Process watchdog:** `C:\Users\whyy\.claude\motogp-guard.ps1` runs in the background during agent work. Every 5 s it gives cargo, rustc, linker, llvm-cov, Codex, project Node and rust-analyzer processes below-normal priority, and pins them to logical cores 0–7, leaving 8–15 free. Start it at the beginning of every session (`& "C:\Users\whyy\.claude\motogp-guard.ps1"` in a background PowerShell).
- **Codex: one run at a time**, read-only reviews only, started directly by the orchestrator (`codex-companion.mjs task`), never several in parallel. Each run starts the `impar-skill-hub` MCP inside WSL, and those processes stay behind. Don't start a new Codex run while one is still running.
- **Devs iterate narrowly:** use `cargo test -p <crate>` while working; run the full workspace gates once at the end. No clippy or llvm-cov during rebases or conflict fixes, because CI runs everything.
- **Coverage (`cargo llvm-cov`) only in QA**, once per issue. It rebuilds everything with instrumentation and is the heaviest step.
- **Stop stale work:** when an issue's code is about to change (for example after a Codex review), stop its running QA. Remove a merged issue's worktree right away.
- **Prefer CI over local:** PR validation, rebases with no code changes and installer builds rely on GitHub Actions, not local builds.

**One branch per unit of work, always.** Every task, fix, translation or doc change gets its own branch from `dev` and its own PR; never mix unrelated changes in the same branch. Parallel work uses separate `git worktree`s inside the repo, under `.worktrees/<issue>` (git-ignored), so branches don't step on each other.

**Parallelize by default; never run issues one by one when they are independent.**
- At any moment, every issue whose dependencies are satisfied must be in progress, each one with its own PO/dev/QA agents in its own worktree (`.worktrees/<issue>`).
- Do not wait for a PR to be merged to start dependent work: stack the new branch on the unmerged parent branch, then rebase it onto `dev` as soon as the parent merges.
- Review (Codex), QA, translation/docs and CI waits run concurrently with development; the orchestrator never idles waiting on a single agent.
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
- **Gates:** QA (Haiku) runs fmt, lint, types, tests, BDD, architecture and coverage. Then Codex reviews. The CI in `.github/workflows/validacao.yml` repeats the same gates.
