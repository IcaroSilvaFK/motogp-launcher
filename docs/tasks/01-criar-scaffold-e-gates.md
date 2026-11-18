---
title: "Crie o scaffold e os gates de validação"
labels: [chore, domain, application, infrastructure, interface, frontend, ci]
depends_on: []
branch: feat/01-criar-scaffold-e-gates
---

## Contexto

Adapta a Task 1 do plano. Prepare Tauri 2, React + TypeScript + Vite e pnpm em `D:\www\motogp-launcher`, no Windows nativo com Rust estável MSVC. Preserve os documentos, o histórico e os workflows existentes: o procedimento antigo de mover/apagar o projeto e executar `git init` não se aplica.

## Escopo (por camada)

- **Domain Rust:** crie `crates/domain`, sem dependências de produção externas, serde, Tauri ou outros crates do projeto. Reserve entidades e regras puras para as próximas issues.
- **Application Rust:** crie `crates/application`, com dependência de produção apenas de `domain`; casos de uso e ports/traits serão introduzidos nas issues que os utilizam. Dependências de testes, como `cucumber`, ficam em `dev-dependencies`.
- **Infrastructure Rust:** crie `crates/infrastructure` para adapters; dependências de fs/JSON, ZIP/SHA-256 e Steam/processos entram aqui quando necessárias, nunca no núcleo.
- **Interface Rust:** mantenha `src-tauri` como membro do workspace Cargo da raiz, com biblioteca `motogp_launcher_lib`, bootstrap e futura composição dos adapters. Fixe os nomes de pacote Cargo como `motogp-domain`, `motogp-application`, `motogp-infrastructure` e `motogp-launcher`, respectivamente, para tornar os comandos de validação reproduzíveis. Configure a janela como MotoGP Launcher, 1100×700, mínimo 900×600, identifier `dev.motogplauncher.app`. Reserve comandos finos, conversão de DTOs e mapeamento de erros.
- **Frontend:** crie `src/domain`, `src/application`, `src/infrastructure` e `src/ui`; React fica na apresentação e nos hooks de aplicação quando necessários. Configure `eslint-plugin-boundaries`: domain não importa camadas externas; application não importa infrastructure/ui; infrastructure implementa ports; ui usa domain/application e não importa infrastructure. Faça a injeção dos adapters em `src/main.tsx`, raiz de composição com exceção explícita e restrita a esse arquivo. Tipos puros ficam em domain; SDKs Tauri, diálogo e eventos ficam em infrastructure.
- **Tooling/CI:** configure rustfmt, Clippy, TypeScript, ESLint/boundaries, Prettier, Vitest + Testing Library e cobertura. `pnpm test` deve terminar em modo não interativo. Declare o package manager e gere o lockfile pnpm. Configure targets BDD Rust `features`, com `harness = false`, `cucumber` como dependência de desenvolvimento e `tests/features/` em cada crate que executa cenários; o frontend usa Vitest para os cenários Gherkin.
- **CI existente:** adapte `.github/workflows/validacao.yml` mantendo os checks obrigatórios **Rust core** e **Frontend**, PRs/pushes para `dev`, `qas`, `main` e build posterior aos gates. Não aceite jobs ignorados como evidência de scaffold validado. Rust core roda no Windows; o frontend pode manter o runner Linux existente. Execute fmt, lint, tipos, unitários, BDD, validação de dependências Cargo e cobertura. Imponha ≥90% de linhas em **cada** crate domain/application e nas respectivas camadas frontend; imponha ≥80% geral por stack. O atual limite Rust geral de 80% sozinho não atende à regra de 90% do núcleo. Não use exclusões amplas para esconder lógica sem teste.
- **Execução:** os comandos locais rodam a partir da raiz do workspace. Use `cargo test --workspace --test features` para os runners BDD, equivalente ao gate `cargo test --test features` com seleção explícita de todo o workspace. Para um único runner, use `cargo test -p motogp-application --test features`, por exemplo. Preserve a geração futura de NSIS/MSI e release, sem antecipar a configuração de assinatura da issue 10.

## Fora de escopo

Importação, perfis, detecção Steam, deploy, sessão e updater. Não instalar ferramentas globalmente se os pré-requisitos já estiverem disponíveis; documentar os faltantes. Não substituir documentos ou workflows por templates vazios, nem executar o roteiro destrutivo do scaffold antigo.

## Critérios de aceite (Gherkin)

Rust: `src-tauri/tests/features/scaffold.feature`, runner `cucumber`; frontend: `src/ui/__tests__/scaffold.test.tsx`, Vitest + Testing Library. As verificações do workspace/tooling são testes de integração da interface, sem I/O nos testes do núcleo. Os cenários de comandos e CI devem executar as verificações reais; a abertura da janela tem smoke test no Windows.

```gherkin
# language: pt
Funcionalidade: Iniciar o projeto com fronteiras e validação obrigatória
  Cenário: Gerar o scaffold preservando o projeto
    Dado que o projeto contém documentos e workflows versionados
    Quando o scaffold é integrado na raiz atual
    Então os documentos e o histórico são preservados
    E o workspace contém domain, application, infrastructure e src-tauri
    E os testes do workspace e do frontend terminam com sucesso

  Cenário: Abrir a aplicação no Windows
    Dado que os pré-requisitos MSVC, Rust, Node, pnpm e WebView2 estão disponíveis
    Quando executo o aplicativo com pnpm tauri dev
    Então vejo a janela MotoGP Launcher nas dimensões especificadas
    E o frontend renderiza sem erros

  Cenário: Rejeitar dependências contra a arquitetura
    Dado um import de ui para infrastructure em uma fixture de lint
    E uma dependência de application para infrastructure em uma fixture Cargo
    Quando executo os validadores de arquitetura
    Então cada violação é rejeitada
    E a raiz de composição autorizada continua válida

  Cenário: Executar todos os gates no CI
    Dado um PR para dev com código Rust e frontend
    Quando o workflow de validação executa
    Então Rust core e Frontend passam executando seus gates
    E falhas de lint, tipos, BDD ou cobertura reprovam a validação
    E a cobertura do núcleo é no mínimo 90 por cento e a geral 80 por cento
```

## Testes TDD (escrever antes)

O plano não nomeava testes desta task. Escreva primeiro as verificações do scaffold, sem inventar regras de negócio para preencher cobertura:

- **Tooling de arquitetura:** `rejects_ui_importing_infrastructure` e `rejects_outward_cargo_dependencies`, com fixtures isoladas que não tornam o projeto principal inválido.
- **Frontend/ui:** `renders_launcher_shell`, smoke de renderização acessível no Vitest.
- **Integração do tooling:** `workspace_members_are_registered` e `coverage_thresholds_reject_insufficient_coverage`, verificando metadata/configuração e o comportamento de falha dos gates. Não são testes unitários de negócio.
- Registre Red → Green → Refactor; enquanto não houver código de núcleo mensurável, documente essa condição sem criar regras artificiais ou relaxar os limites para as próximas issues.

## Definition of Done

- [ ] Testes escritos antes da implementação; evidências Red → Green → Refactor e commits `test:` antes ou junto de `feat:`/`chore:`.
- [ ] Todos os cenários acima têm testes correspondentes; smoke da janela executado no Windows.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: runners `cargo test --workspace --test features` e cenários Vitest verdes, sem exigir jogo ou rede.
- [ ] Gate 6: metadata Cargo e boundaries confirmam dependências para dentro; só dev-dependencies podem conter o tooling BDD do núcleo.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` aplicam ≥90% domain/application e ≥80% geral; ausência inicial de linhas do núcleo explicitada, sem bypass quando houver implementação.
- [ ] Gate 8: critérios de aceite rastreados aos testes; **Rust core** e **Frontend** verdes no CI, sem skip.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes de abrir o PR.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` usando o número real após publicação. Promoções somente `dev → qas → main`; sem commit direto nessas branches.
