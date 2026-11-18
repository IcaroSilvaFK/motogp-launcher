# Issues do MotoGP Launcher

Estas 11 issues adaptam as 10 tasks do plano de 18/11/2026 ao fluxo de TDD, BDD e Arquitetura Limpa. A issue 10 adiciona o auto-update; a Task 10 original passa a ser a issue 11. Os arquivos estão preparados para publicação futura: nenhum número de issue GitHub foi presumido.

| NN | Título | Depende de | Branch |
| --- | --- | --- | --- |
| 01 | [Crie o scaffold e os gates de validação](01-criar-scaffold-e-gates.md) | — | `feat/01-criar-scaffold-e-gates` |
| 02 | [Persista dados JSON e trate erros sem perda](02-persistir-dados-e-erros.md) | 01 | `feat/02-persistir-dados-e-erros` |
| 03 | [Importe mods para a biblioteca local](03-importar-biblioteca-de-mods.md) | 02 | `feat/03-importar-biblioteca-de-mods` |
| 04 | [Gerencie perfis e a ordem dos mods](04-gerenciar-perfis-ordenados.md) | 02 | `feat/04-gerenciar-perfis-ordenados` |
| 05 | [Detecte instalações MotoGP pela Steam](05-detectar-instalacoes-do-jogo.md) | 02 | `feat/05-detectar-instalacoes-do-jogo` |
| 06 | [Aplique e limpe mods com manifesto seguro](06-aplicar-e-limpar-mods.md) | 03 | `feat/06-aplicar-e-limpar-mods` |
| 07 | [Inicie o jogo e acompanhe a sessão](07-iniciar-e-acompanhar-sessao.md) | 05 | `feat/07-iniciar-e-acompanhar-sessao` |
| 08 | [Integre comandos Tauri e recuperação de sessão](08-integrar-comandos-e-recuperacao.md) | 03, 04, 05, 06, 07 | `feat/08-integrar-comandos-e-recuperacao` |
| 09 | [Construa a interface do launcher](09-construir-interface-do-launcher.md) | 08 | `feat/09-construir-interface-do-launcher` |
| 10 | [Atualize o aplicativo pela interface](10-atualizar-aplicativo-pelo-front.md) | 09 | `feat/10-atualizar-aplicativo-pelo-front` |
| 11 | [Valide o build Windows e a sessão real](11-validar-build-e-jogo-real.md) | 10 | `feat/11-validar-build-e-jogo-real` |

As dependências no frontmatter são referências locais de dois dígitos; transitivas estão implícitas. Após 02, as issues 03/04/05 podem avançar independentemente; 06 exige 03 e 07 exige 05. Isso indica possibilidade de execução, sem iniciar agentes ou desenvolvimento neste planejamento. 08 integra as cinco entregas, 09 entrega a UI, 10 adiciona atualização e 11 valida o conjunto.

## Fontes e precedência

Leia [CLAUDE.md](../../CLAUDE.md), [workflow de validação](../workflow-validacao.md), [auto-update](../auto-update.md), [design](../superpowers/specs/2026-11-18-motogp-launcher-design.md) e [plano original](../superpowers/plans/2026-11-18-motogp-launcher.md). O pedido atual e as regras novas substituem a organização monolítica antiga e o não objetivo de auto-update. O workflow existente [.github/workflows/validacao.yml](../../.github/workflows/validacao.yml) foi considerado para evitar recriar jobs já presentes.

## Decisões de adaptação

- **Camadas:** workspace Cargo na raiz com `crates/domain`, `crates/application`, `crates/infrastructure`, `src-tauri`; modelos puros no domain, ports/casos de uso em application, serde/fs/ZIP/Steam/processos em adapters, DTOs/composição/comandos finos no Tauri. Dependências de produção de application limitam-se a domain; `cucumber` e outros auxiliares de teste são dev-dependencies, sem exceção para I/O no código do núcleo.
- **Frontend:** `src/domain|application|infrastructure|ui`, com `eslint-plugin-boundaries`; injeção em `src/main.tsx`, exceção de composição restrita ao arquivo. Tipos puros não importam o SDK e ui nunca importa infrastructure.
- **BDD:** Rust usa `tests/features/*.feature` no crate responsável e target `features` com `harness = false`/crate cucumber. Execute todos com `cargo test --workspace --test features`, ou selecione por `-p motogp-domain`, `-p motogp-application`, `-p motogp-infrastructure` e `-p motogp-launcher`, nomes fixados na issue 01. Frontend conserva o Gherkin em `tests/features/frontend/` e o executa como describe/it Vitest + Testing Library, conforme workflow; não usa cucumber Rust para componentes React. Cada cenário deve ter teste rastreável, inclusive integração quando não for unitário.
- **TDD:** os **35 nomes de testes originais** das Tasks 2–8 foram preservados nas issues 02–08. Testes fs/ZIP/Steam com `tempfile` continuam como contratos/integração de infrastructure; regras/casos de uso são testados sem disco/rede/Tauri com fakes. Tasks 1, 9 e 10 originais não nomeavam unitários; as issues correspondentes propõem verificações novas, sem alegar que já existem.
- **Gates:** 01 adapta o CI existente para que Rust core e Frontend executem todos os gates e imponham ≥90% em cada domain/application e ≥80% geral por stack. A ausência inicial de linhas de núcleo é explicitada, nunca substituída por regra de negócio fictícia ou desativação dos limites posteriores.
- **Consistência:** o App ID é validado antes de deploy, conciliando Task 8 com a exigência manual da Task 9 de não criar `~mods` quando o ID falta. Vanilla é protegido na aplicação, além da UI; excluir o perfil ativo volta a Vanilla. JSON de zero bytes é corrupção; somente arquivo ausente ou coleção JSON válida vazia permitem semear perfis.
- **Arquivos e crash:** a gravação do manifesto por arquivo foi mantida, mas a issue 06 exige recuperação testada nos intervalos de interrupção, com rastreio de propriedade e sem apagar por prefixo. Colisões de nomes ZIP, sobrescrita no fallback de cópia, manifesto com caminho inseguro e limpeza parcial têm critérios explícitos. A recuperação de crash ocorre na próxima abertura; não há promessa de limpeza com o launcher encerrado.
- **Updater:** exige clique, assinatura válida e sessão limpa antes de instalar/reiniciar. Owner/repo, acesso público ao feed, chave pública e secrets são pré-requisitos externos a confirmar na implementação da issue 10; nenhum valor foi inventado e nenhuma chave/token é entregue nestes arquivos. Sem feed acessível não existe aceite de atualização real; não presuma acesso anônimo à release privada. `latest.json` carrega as assinaturas dos pacotes, sem exigir uma assinatura separada do próprio JSON.
- **Build:** workspace altera o destino padrão para `target/release`, respeitando configuração efetiva. O limite objetivo do plano é executável <15 MB; registre também tamanhos dos instaladores e avalie a meta do design de instalador leve. O bootstrap WebView2 pode afetar o instalador. Testes reais de jogo/atualização são obrigatórios na issue 11, em ambiente Windows controlado, além da suíte determinística do CI.

## Publicação e fluxo futuro

Publique futuramente cada corpo como issue, aplique labels e substitua referências locais por links/números reais quando disponíveis. `NN` é ordem do plano, não número GitHub. O campo `branch: feat/<NN>-<slug>` segue o formato solicitado; se a numeração GitHub for diferente, vincule a issue real explicitamente na branch/PR e use `Closes #<nº-real>`, sem assumir que NN coincide com ela. Essa escolha atende ao formato atual mesmo que o exemplo de CLAUDE.md use número da issue no nome da branch.

As branches de task saem de `dev`. Desenvolvedores seguem TDD, QA Haiku executa os oito gates e Codex revisa/aplica melhorias antes da abertura do PR para `dev`. Promoção ocorre por PR `dev → qas → main`; hotfix segue o fluxo de CLAUDE.md. Não criar commits diretos nessas branches nem promover sem os checks Rust core e Frontend.

Este lote contém apenas especificações Markdown. Não executa scaffold, testes do produto, comandos Git/gh, publicação de issues, configuração de secrets ou releases.
