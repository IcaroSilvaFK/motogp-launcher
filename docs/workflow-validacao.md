# Workflow de validação: TDD + BDD + Arquitetura Limpa

Obrigatório para **toda task**. Nenhum PR é aberto sem passar por todas as etapas.

Stack: Tauri 2 + Rust (core), React + TypeScript + Vite (frontend), pnpm.

---

## 1. BDD: especificar o comportamento (Codex, na issue)

Toda issue traz os critérios de aceite em **Gherkin**:

```gherkin
Funcionalidade: Instalar mod
  Cenário: Mod válido é adicionado à biblioteca
    Dado que o usuário tem um perfil ativo
    Quando ele importa o arquivo "mod.zip"
    Então o mod aparece na biblioteca
    E o arquivo library.json é atualizado
```

- Os cenários viram arquivos `.feature` no repo:
  - Rust: `tests/features/*.feature`, executados com o crate [`cucumber`](https://crates.io/crates/cucumber).
  - Frontend: os cenários viram `describe("Dado…") / it("Quando… Então…")` no Vitest + Testing Library.
- Se um cenário não pode ser testado, a issue está mal definida. Ela volta para o Codex.

## 2. TDD: Red → Green → Refactor (Dev, Sonnet)

Para cada cenário/regra:

1. **Red:** escrever o teste primeiro e rodar para ver ele **falhar** pelo motivo certo.
2. **Green:** escrever o mínimo de código para o teste passar.
3. **Refactor:** limpar com os testes verdes.

Regras:
- Os commits mostram a ordem: o commit `test:` vem antes ou junto do `feat:`. Código de produção sem teste que o cubra é rejeitado.
- O domínio e os casos de uso são testados **sem** Tauri, sistema de arquivos ou rede (só fakes dos ports).
- Cobertura mínima: **domain/application ≥ 90%**, geral ≥ 80%.

## 3. Arquitetura Limpa: estrutura e regra de dependência

As dependências apontam sempre **para dentro**: `interface → infrastructure → application → domain`.

### Rust (workspace Cargo, a separação é imposta pelo compilador)

```
crates/
  domain/          # entidades, value objects, regras. Sem deps externas (nem serde de IO, nem tauri)
  application/     # casos de uso + ports (traits): ModRepository, FileSystem, Downloader…
  infrastructure/  # adapters que implementam os ports (fs, http, json)
src-tauri/         # camada de interface: #[tauri::command] finos que só chamam casos de uso
```

- `domain` não depende de nenhum outro crate do projeto.
- `application` depende só de `domain`.
- Os comandos Tauri não têm regra de negócio: convertem o DTO, chamam o caso de uso e mapeiam o erro.

### Frontend (React + TS)

```
src/
  domain/          # tipos e regras puras
  application/     # hooks/casos de uso que dependem de interfaces (ports)
  infrastructure/  # adapters: invoke() do Tauri, storage
  ui/              # componentes React (só apresentação + chamada de casos de uso)
```

- Fronteiras validadas com `eslint-plugin-boundaries` (ou `dependency-cruiser`): `ui` não importa `infrastructure` diretamente, e `domain` não importa nada de fora.

## 4. Gates de validação (QA Haiku → Codex)

O **QA (Haiku)** roda e confere, em ordem:

| # | Gate | Comando |
|---|------|---------|
| 1 | Formatação | `cargo fmt --check` · `pnpm prettier --check .` |
| 2 | Lint | `cargo clippy --all-targets -- -D warnings` · `pnpm lint` |
| 3 | Tipos | `pnpm tsc --noEmit` |
| 4 | Testes unitários (TDD) | `cargo test --workspace` · `pnpm test` |
| 5 | Cenários BDD | `cargo test --test features` · cenários Vitest |
| 6 | Arquitetura | deps dos crates conferidas + `pnpm lint` (boundaries) |
| 7 | Cobertura | `cargo llvm-cov` · `pnpm test --coverage` (metas acima) |
| 8 | Critérios de aceite | cada cenário Gherkin da issue tem teste correspondente e passando |

Se algum gate falhar, a entrega volta para o dev com o relatório do QA.

O **Codex** revisa depois:
- se a ordem TDD é visível nos commits;
- se a regra de dependência e as responsabilidades por camada foram respeitadas;
- se os testes estão bons: comportamento em vez de implementação, sem mocks excessivos;
- melhorias e refactors sugeridos, que são aplicados antes do PR.

## 5. Checklist do PR

- [ ] Issue com cenários Gherkin vinculada (`Closes #<nº>`)
- [ ] Testes escritos antes do código (Red → Green → Refactor)
- [ ] Cenários BDD implementados e passando
- [ ] Domain/application sem dependência de infra/Tauri
- [ ] Todos os gates verdes (local e CI)
- [ ] QA aprovado + revisão do Codex aplicada

O CI ([.github/workflows/validacao.yml](../.github/workflows/validacao.yml)) roda os mesmos gates em todo PR.

---

## 6. Scaffold (issue #1): how the gates run

Run everything from the workspace root. `pnpm` is declared through `packageManager` in `package.json` (`corepack pnpm ...` works without a global install).

| Gate | Rust | Frontend |
|------|------|----------|
| 1 Formatting | `cargo fmt --all --check` | `pnpm prettier --check .` (docs/ and `*.md` are ignored on purpose) |
| 2 Lint | `cargo clippy --workspace --all-targets -- -D warnings` | `pnpm lint` (ESLint + `eslint-plugin-boundaries`) |
| 3 Types | built into `cargo` | `pnpm tsc --noEmit` |
| 4 Unit tests | `cargo test --workspace` | `pnpm test` (`vitest run`, non-interactive) |
| 5 BDD | `cargo test --workspace --test features` (or `-p motogp-launcher --test features`) | Vitest scenarios (`src/ui/__tests__`, `tooling/__tests__`, Gherkin in `tests/features/frontend/`) |
| 6 Architecture | `cargo test -p motogp-launcher --test architecture` (reads `cargo metadata`) | `pnpm lint` (boundaries) |
| 7 Coverage | `cargo llvm-cov --workspace --json --summary-only --output-path coverage-rust.json --ignore-filename-regex '(^\|[\\/])\.worktrees[\\/]\|src-tauri[\\/]src[\\/](main\|bootstrap)\.rs'` then `node scripts/check-rust-coverage.mjs coverage-rust.json` | `pnpm test --coverage` |

Limits live in `tooling/coverage-thresholds.json` (`core` = 90, `overall` = 80) and are shared by Vitest, the Rust coverage script and the BDD checks.

### Initial coverage condition (no core lines yet)

`motogp-domain`, `motogp-application`, `src/domain` and `src/application` have no executable lines in issue #1. No fictitious business rule was added and no limit was relaxed:

- Rust: `scripts/check-rust-coverage.mjs` prints an explicit `NOTE ... no measurable lines yet` for an empty core crate and enforces 90% per crate as soon as the report contains lines for it.
- Frontend: Vitest thresholds (`src/domain/**` and `src/application/**` at 90%, 80% overall) apply to any code that lands there.
- The only coverage exclusions are narrow: the Tauri bootstrap (`src-tauri/src/main.rs`, `bootstrap.rs`, only runs with a real window), `src/main.tsx` (composition root that mounts React) and test/declaration files.
- `coverage_thresholds_reject_insufficient_coverage` proves that insufficient reports fail the gate.

### Scenario traceability (issue #1)

| Scenario | Test |
|----------|------|
| Generate the scaffold preserving the project | `src-tauri/tests/features/scaffold.feature` (cucumber) + `workspace_members_are_registered` |
| Open the application on Windows | cucumber scenario `Validate the Windows application configuration` checks `tauri.conf.json` (title, 1100x700, min 900x600, identifier); `renders_launcher_shell` checks rendering; the actual window opening is the manual smoke below |
| Reject dependencies that violate the architecture | `rejects_ui_importing_infrastructure` (Vitest, real ESLint on `tests/fixtures/boundaries`), `rejects_outward_cargo_dependencies` (`src-tauri/tests/architecture.rs`, fixtures in `src-tauri/tests/fixtures`) |
| Run all gates in CI | cucumber step + `tooling/__tests__/ci-workflow.test.ts` inspect `.github/workflows/validacao.yml`; the CI itself runs the gates |

### Manual smoke: `pnpm tauri dev`

Prerequisites: MSVC build tools, Rust stable (MSVC), Node 22, pnpm (corepack), WebView2 Runtime. Run `pnpm tauri dev` and confirm the "MotoGP Launcher" window opens with a 1100x700 client area (minimum 900x600) and shows the "MotoGP Launcher" heading. Verified on Windows 11 during issue #1.
