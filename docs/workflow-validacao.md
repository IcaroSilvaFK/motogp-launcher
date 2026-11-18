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
