---
title: "Construa a interface do launcher"
labels: [feature, frontend]
depends_on: ["08"]
branch: feat/09-construir-interface-do-launcher
---

## Contexto

Adapta a Task 9. Entregue Biblioteca, Perfis, Configurações e controle da sessão em português, com React + TypeScript. Substitua o `src/api.ts` monolítico do plano por tipos puros, ports, casos de uso e adapters separados.

## Escopo (por camada)

- **Frontend/domain:** defina `Config`, `GameInfo`, `ModEntry`, `Profile` e `SessionEvent` sem SDK Tauri; mantenha campos de resposta snake_case (`game_dir`, `mods_dir`, `app_id`, `mod_ids`, `size_bytes`, `active_profile`) conforme IPC da issue 08.
- **Frontend/application:** defina ports para operações do launcher, seleção de arquivos/pasta e eventos de sessão/drop. Implemente casos de uso/hooks para carregar biblioteca/perfis/jogo, importar/remover, editar ordem, selecionar perfil e jogar. Injete adapters; trate erros e estados de carregamento/sessão com fakes nos testes. Recarregue os dados após mutações, inclusive para refletir importações anteriores concluídas quando uma importação em lote falhar. Descarte listeners ao desmontar.
- **Frontend/infrastructure:** implemente adapter tipado por comando com `invoke`, argumentos camelCase e respostas snake_case. Isole `listen("session")`, `getCurrentWebview().onDragDropEvent` e plugin-dialog nesta camada. No diálogo de importação, permita múltiplos `.pak/.utoc/.ucas/.zip`; no de configuração, selecione diretório. Cancelamento não é erro nem dispara importação.
- **Frontend/ui:** coloque componentes/layout em `src/ui`, incluindo telas Biblioteca, Perfis e Configurações e o App. Sidebar com navegação, seletor de perfil ativo e Jogar desabilitado sem jogo configurado ou durante sessão iniciando/ativa. Mostre estados "Rodando…", "Sessão encerrada — N arquivos removidos" e "O jogo não abriu em 2 min; mods removidos" a partir do evento, sem anunciar limpeza em caso de erro.
- **Biblioteca:** exiba nome, tamanho e quantidade de arquivos, importar por diálogo/drop e remover com recarga de perfis afetados.
- **Perfis:** crie/renomeie/exclua, apresente checkboxes de mods e controles ↑/↓ acessíveis; persista na ordem escolhida. Vanilla não oferece exclusão, além da proteção backend já entregue.
- **Configurações:** exiba nome, `mods_dir` e App ID do jogo; liste detecções com "Usar este" e ofereça "Escolher pasta…". Pasta válida sem App ID pode ser configurada, mas Jogar deve receber o erro do backend sem aplicar arquivos.
- **Composição e acessibilidade:** `src/main.tsx` injeta adapters nos casos de uso; componentes ui nunca importam infrastructure. Inputs/botões têm nomes acessíveis, navegação por teclado e foco utilizável; aviso de erro no topo com anúncio acessível. Remova a apresentação do template.

## Fora de escopo

Auto-update (issue 10), editor de mods, navegação online, polling de processos em React e regras de negócio duplicadas na UI. Não criar `src/api.ts` misturando tipos, SDK e regras.

## Critérios de aceite (Gherkin)

Fonte dos cenários: `tests/features/frontend/interface.feature`. No frontend, conforme o workflow, execute-os como `describe("Dado…")` / `it("Quando… Então…")` em Vitest + Testing Library: `src/ui/__tests__/launcher.test.tsx` e testes de application. Este `.feature` é a especificação rastreável; não adicione cucumber Rust para testar React. Os comportamentos backend correspondentes permanecem nos runners cucumber das issues anteriores.

```gherkin
# language: pt
Funcionalidade: Usar o launcher pela interface
  Cenário: Configurar uma pasta Unreal sem Steam
    Dado a pasta falsa "C:/temp/MotoGPFake/MotoGPFake/Content/Paks"
    Quando escolho "C:/temp/MotoGPFake" em Configurações
    Então vejo mods_dir terminando em "Content/Paks/~mods"
    E vejo o App ID como não encontrado

  Cenário: Usar um jogo detectado
    Dado que a detecção retorna um MotoGP com App ID
    Quando clico em "Usar este"
    Então a configuração do jogo é persistida
    E o jogo atual é atualizado na tela

  Cenário: Importar pelo diálogo sem duplicar
    Dado um pak de teste ainda fora da biblioteca
    Quando o seleciono em Importar
    Então a biblioteca mostra seu nome, tamanho e quantidade de arquivos
    Quando importo novamente o mesmo pak
    Então continua existindo uma única entrada para o mod

  Cenário: Importar por arrastar e soltar
    Dado a tela Biblioteca aberta
    Quando recebo um evento drop com arquivos suportados
    Então os caminhos são enviados ao caso de uso de importação
    E a lista é recarregada

  Cenário: Cancelar seleção e remover listener
    Dado um diálogo aberto e listeners ativos na tela
    Quando cancelo o diálogo e depois desmonto a tela
    Então nenhuma importação é solicitada
    E os listeners são liberados sem callbacks duplicados

  Cenário: Criar perfil e ordenar mods
    Dado mods A e B na biblioteca
    Quando crio o perfil "Teste", marco os mods e movo B antes de A
    Então salvo mod_ids na ordem B, A
    E a ordem permanece após recarregar o perfil

  Cenário: Renomear, excluir e proteger perfis
    Dado um perfil comum e Vanilla
    Quando renomeio o perfil comum e depois o excluo
    Então a lista reflete as duas alterações
    E Vanilla não oferece ação de exclusão

  Cenário: Remover mod da biblioteca e dos perfis
    Dado um mod listado e referenciado em um perfil
    Quando o removo pela Biblioteca
    Então ele deixa de aparecer na biblioteca e na seleção do perfil

  Cenário: Avisar erro ao jogar sem App ID
    Dado o perfil "Teste" ativo e um jogo configurado sem App ID
    Quando clico em Jogar
    Então vejo aviso contendo "App ID do jogo não encontrado"
    E nenhum arquivo é criado em ~mods

  Cenário: Controlar disponibilidade de Jogar
    Dado o aplicativo sem jogo configurado
    Então Jogar está desabilitado
    Quando configuro um jogo e solicito iniciar uma sessão
    Então Jogar fica desabilitado durante início e execução

  Cenário: Mostrar eventos de sessão
    Dado uma sessão solicitada
    Quando recebo session com estado "started"
    Então vejo "Rodando…"
    Quando recebo session com estado "ended" e cleaned 2
    Então vejo "Sessão encerrada — 2 arquivos removidos"
    E Jogar volta a ficar disponível

  Cenário: Mostrar que o jogo não abriu
    Dado uma sessão solicitada
    Quando recebo session com estado "never-started" após limpeza concluída
    Então vejo "O jogo não abriu em 2 min; mods removidos"

  Cenário: Mostrar erros de operação
    Dado uma operação cujo port retorna erro em português
    Quando a operação termina
    Então vejo o erro no aviso acessível do topo
    E não vejo confirmação falsa de sucesso
```

## Testes TDD (escrever antes)

O plano não nomeava testes unitários frontend. Escreva antes dos componentes/casos de uso:

- **Frontend/application**, ports fake: `loads_launcher_state`, `imports_and_refreshes_library`, `reimport_keeps_single_mod`, `remove_mod_refreshes_profiles`, `saves_profile_mod_order`, `sets_active_profile`, `handles_session_events`, `operation_error_is_exposed` e `listeners_are_released_on_dispose`.
- **Frontend/ui**, Vitest + Testing Library com serviços de aplicação fake: `settings_accepts_fake_unreal_directory`, `selects_detected_game`, `library_shows_mod_metadata`, `imports_from_dialog_and_drop`, `cancelled_dialog_does_not_import`, `creates_renames_and_deletes_profile`, `vanilla_delete_is_unavailable`, `profile_order_can_be_changed_with_keyboard`, `play_is_disabled_without_game_or_during_session`, `missing_app_id_error_is_visible`, `session_status_uses_cleaned_count` e `never_started_status_is_visible`.
- **Frontend/infrastructure**, SDKs mockados só na fronteira: `invoke_uses_expected_commands_and_argument_names`, cobrindo todos os comandos da issue 08; `session_and_drop_adapters_unsubscribe` e `dialog_filters_supported_mod_extensions`.
- O teste de UI verifica que o caso de uso recusa Jogar sem App ID; a prova de que `~mods` permanece intacto fica no teste de application Rust da issue 08, sem acessar disco de dentro do teste React.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado para hooks/casos de uso e componentes; os testes antecedem o código.
- [ ] Todos os cenários Gherkin mapeados a describe/it em Vitest; runners cucumber backend continuam verdes.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` e cenários Vitest verdes.
- [ ] Gate 6: boundaries rejeitam ui → infrastructure; SDK Tauri só em adapters/bootstrap; dependências Cargo preservadas.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios comprovados, smoke `pnpm tauri dev` no Windows com pasta falsa e verificação de teclado/foco; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes do PR.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
