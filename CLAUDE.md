# motogp-launcher

## Memória e preferências

Sempre salvar memórias, decisões e preferências **localmente no projeto** (neste `CLAUDE.md` ou em `docs/`), nunca só na memória global do usuário.

## Fluxo de desenvolvimento (multi-agente)

Todo trabalho de desenvolvimento segue este pipeline de papéis:

1. **Agente especializado geral (o chat)** — ponto de contato com o usuário; entende o pedido, orquestra o fluxo e consolida o resultado.
2. **Codex** — organiza e elabora as tasks (quebra o trabalho em tarefas detalhadas).
3. **Agente PO (1)** — coordena os desenvolvedores: distribui as tasks e acompanha a entrega.
4. **Agentes desenvolvedores (N)** — implementam as tasks, em paralelo quando possível. **Modelo: Sonnet** (`model: "sonnet"`).
5. **Agentes de QA (N)** — testam e validam o que os devs entregaram. **Modelo: Haiku** (`model: "haiku"`).
6. **Codex** — revisão final e melhorias do que foi implementado.

Regra: o chat não implementa direto. Demanda de desenvolvimento → Codex gera as tasks → PO dispara os devs (Sonnet) → QA valida (Haiku) → Codex revisa → chat reporta ao usuário.

## Fluxo de Git / issues (obrigatório para toda task)

1. **Codex elabora** a task com todos os detalhes (contexto, escopo, critérios de aceite).
2. **Publica uma issue** no repositório remoto (`gh issue create`) com esse detalhamento.
3. **Dev abre uma branch** a partir de `DEV`, vinculada à issue, para manter o histórico (ex.: `feat/<nº-issue>-<slug>`).
4. **Dev finaliza a task** na branch.
5. A entrega **passa por toda a revisão** (QA Haiku + revisão/melhorias do Codex).
6. Só depois: **abre o PR para `DEV`** referenciando a issue (`Closes #<nº>`) e **fecha a issue**.

### Branches padrão

| Branch | Ambiente | Recebe de |
|--------|----------|-----------|
| `DEV` | Desenvolvimento (integração) | PRs das branches de task (`feat/…`, `fix/…`) |
| `QAS` | Homologação / QA | PR de promoção `DEV → QAS` |
| `main` | Produção (PRD) | PR de promoção `QAS → main` |

- Ninguém commita direto em `DEV`, `QAS` ou `main`: tudo entra via PR.
- O pipeline gera o **instalador Windows** (setup `.exe` NSIS e `.msi`) depois que todos os gates passam. Em `DEV`/`QAS` ele fica como artefato do pipeline (30 dias). Em `main` vira uma **GitHub Release** `v<versão>` (versão vinda do `tauri.conf.json`), junto com o `latest.json` do **auto-update**: o app instalado se atualiza pelo front. Setup em [docs/auto-update.md](docs/auto-update.md).
- Promoção só segue em frente (`DEV → QAS → main`). Hotfix sai de `main` e volta por PR para `main`, `QAS` e `DEV`.

## Workflow de validação: TDD + BDD + Arquitetura Limpa (obrigatório)

Detalhes em [docs/workflow-validacao.md](docs/workflow-validacao.md). Resumo:

- **BDD:** toda issue tem critérios de aceite em Gherkin, e cada cenário vira teste.
- **TDD:** Red → Green → Refactor; teste sempre antes do código. Cobertura ≥ 90% em domain/application e ≥ 80% no geral.
- **Arquitetura Limpa:** dependências só para dentro (`interface → infrastructure → application → domain`). Rust em crates separados (`domain`, `application`, `infrastructure`, `src-tauri`). Frontend em `domain/application/infrastructure/ui`, com fronteiras checadas no lint.
- **Gates:** o QA (Haiku) roda fmt, lint, tipos, testes, BDD, arquitetura e cobertura. Depois o Codex revisa. O CI em `.github/workflows/validacao.yml` repete os mesmos gates.
