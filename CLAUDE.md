# motogp-launcher

## Language

**File names and file contents are always in English**: code, identifiers, comments, test names, Gherkin `.feature` files (Feature/Scenario/Given/When/Then), commit messages, branch names, docs, issues/PRs and error messages. Only the chat with the user stays in Portuguese.

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
- **Disk:** each worktree has its own `target/`. Run `cargo clean` and remove the worktree as soon as its PR merges.

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
