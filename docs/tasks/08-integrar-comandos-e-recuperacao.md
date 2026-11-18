---
title: "Integre comandos Tauri e recuperação de sessão"
labels: [feature, application, infrastructure, interface]
depends_on: ["03", "04", "05", "06", "07"]
branch: feat/08-integrar-comandos-e-recuperacao
---

## Contexto

Adapta a Task 8. O plano antigo colocava resolução de perfil e orquestração de sessão em `commands.rs`; elas passam para application. Os comandos Tauri só convertem DTOs, chamam casos de uso e mapeiam respostas/erros.

## Escopo (por camada)

- **Application:** implemente os casos de uso ainda necessários para configurar jogo, perfil ativo, importação múltipla, remoção de referências e iniciar/recuperar sessão. Mova `resolve_profile_mods` para application: preserve a ordem do perfil e ignore IDs ausentes na biblioteca. Valide jogo e App ID **antes** de aplicar mods; sem App ID, retorne o erro da issue 07 sem tocar em `~mods`.
- **Orquestração:** bloqueie sessões concorrentes desde o pedido de abertura até terminar a limpeza, inclusive enquanto o jogo ainda está iniciando. Resolva perfil, aplique mods, abra Steam e acompanhe eventos. Em Ended/NeverStarted, limpe e publique `session` com estado e número real de arquivos removidos; em Started, apenas publique. Falhas de deploy/opener disparam compensação e liberam o bloqueio quando for seguro; falhas de limpeza preservam manifesto/pendência e são reportadas, sem anunciar sucesso falso.
- **Infrastructure:** componha repositórios e adapters já entregues; forneça implementação do bloqueio de sessão, execução em background e publicação de eventos por ports, sem levar Tauri para application. Implemente a ponte de abertura Steam com o plugin na interface, onde `AppHandle` está disponível.
- **Interface:** crie `src-tauri/src/commands.rs` fino e o composition root no bootstrap. Resolva `app.path().app_data_dir()` e injete os adapters/estado; antes de aceitar operações novas, execute o caso de uso de recuperação do manifesto. Registre a quantidade limpa; erro de recuperação é visível e bloqueia nova aplicação até ser resolvido.
- **Contrato IPC:** preserve os comandos `get_config`, `detect_games`, `set_game_dir(path)`, `get_game`, `list_mods`, `import_mods(paths)`, `remove_mod(id)`, `list_profiles`, `create_profile(name)`, `rename_profile(id, name)`, `delete_profile(id)`, `set_profile_mods(id, modIds)`, `set_active_profile(id)` e `play(profileId)`. DTOs retornados mantêm snake_case; os argumentos JS usam camelCase. `import_mods` interrompe na primeira falha e informa erro; importações anteriores concluídas permanecem visíveis após recarga. `remove_mod` remove referências em todos os perfis, reportando falhas sem simular conclusão.
- **Consistência:** `set_game_dir` inspeciona antes de salvar; `get_game` retorna None apenas se não configurado, sem esconder erro de inspeção de caminho configurado. Valide perfil ativo; ao excluir o perfil ativo, restaure a seleção Vanilla. Proteja Vanilla também via IPC.
- **Eventos/plugins:** serialize `SessionEvent { state, cleaned }`, com estados `started`, `ended`, `never-started`; falhas seguem canal de erro visível e não evento de conclusão falsa. Registre dialog/opener e capacidades mínimas, incluindo abertura restrita a `steam://*`, sem permissão irrestrita de URL/processo.

## Fora de escopo

Componentes React, updater, novo motor de deploy ou regras dentro de comandos. Não duplicar o watcher na interface.

## Critérios de aceite (Gherkin)

Arquivo principal: `crates/application/tests/features/comandos-e-recuperacao.feature`, runner `cucumber`, com ports fake. Contratos IPC/bootstrap: `src-tauri/tests/features/interface-tauri.feature`, runner `cucumber` com ambiente de teste Tauri; sem exigir jogo real nos gates.

```gherkin
# language: pt
Funcionalidade: Expor casos de uso e recuperar sessões pelo launcher
  Cenário: Resolver mods em ordem ignorando referências ausentes
    Dado um perfil com IDs "b", "gone", "a" e biblioteca com A e B
    Quando resolvo os mods do perfil
    Então recebo B e A nessa ordem

  Cenário: Recuperar aplicação anterior na inicialização
    Dado um manifesto pendente e um arquivo manual em ~mods
    Quando o launcher inicia
    Então a recuperação limpa somente os arquivos aplicados pelo launcher
    E registra a quantidade removida antes de aceitar nova sessão

  Cenário: Impedir sessão simultânea
    Dado uma sessão iniciando ou rodando
    Quando recebo outro pedido Jogar
    Então retorno "o jogo já está rodando"
    E não aplico nem abro o jogo novamente

  Cenário: Validar pré-requisitos antes do deploy
    Dado um jogo não configurado ou sem App ID
    Quando recebo o pedido Jogar
    Então retorno o erro correspondente em português
    E nenhum arquivo ou diretório é criado em ~mods
    E uma tentativa futura continua possível

  Cenário: Concluir sessão com limpeza e evento
    Dado uma sessão com arquivos aplicados
    Quando o watcher sinaliza Ended ou NeverStarted
    Então a limpeza é executada
    E o evento session contém o estado e a quantidade efetivamente removida
    E uma nova sessão pode ser iniciada após a limpeza completa

  Cenário: Emitir início sem limpar prematuramente
    Dado uma sessão com mods aplicados
    Quando o watcher sinaliza Started
    Então emito session com estado "started"
    E os arquivos permanecem aplicados

  Cenário: Compensar falha ao abrir a Steam
    Dado mods aplicados para uma tentativa de sessão
    Quando o adapter de abertura retorna erro
    Então tento limpar somente os arquivos dessa sessão
    E reporto o erro sem manter bloqueio após compensação bem-sucedida

  Cenário: Não esconder falha de recuperação
    Dado que a limpeza do manifesto falha na inicialização ou no término
    Quando a operação retorna
    Então reporto a falha e retenho as pendências recuperáveis
    E não anuncio sessão limpa nem permito novo deploy inseguro

  Cenário: Manter perfis coerentes ao remover dados
    Dado um mod referenciado em perfis e um perfil comum ativo
    Quando removo o mod e excluo o perfil ativo
    Então as referências ao mod são removidas de todos os perfis
    E o perfil ativo passa a ser Vanilla

  Cenário: Preservar contrato dos comandos
    Dado os comandos registrados e ports de aplicação controlados
    Quando envio argumentos camelCase pelo IPC
    Então cada comando encaminha a operação esperada
    E respostas usam os campos snake_case documentados
    E erros são apresentados em português sem regra de negócio no comando
```

## Testes TDD (escrever antes)

- Preserve `resolve_profile_mods_keeps_order_and_skips_missing` em **crates/application**, e não em commands.rs como no plano antigo.
- **Application**, unitários com fakes: `play_without_game_does_not_deploy`, `play_without_app_id_does_not_deploy`, `concurrent_play_is_rejected`, `startup_recovers_previous_deployment`, `ended_session_cleans_and_emits_count`, `never_started_session_cleans_and_releases_lock`, `started_event_does_not_clean`, `opener_failure_rolls_back_deployment`, `cleanup_failure_keeps_recovery_pending`, `remove_mod_clears_profile_references` e `deleting_active_profile_selects_vanilla`.
- **Interface/src-tauri**, testes de contrato: `commands_preserve_camel_case_arguments_and_snake_case_dtos`, cobrindo cada comando listado com fakes, e `setup_runs_recovery_before_accepting_play`. São testes da fronteira, sem regra duplicada no comando.
- **Application:** `batch_import_stops_at_first_error_and_keeps_completed_imports`, verificando o contrato de falha parcial do plano.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado e nome original preservado na camada correta; 35 testes originais das issues 02–08 continuam presentes/passando.
- [ ] Todos os cenários têm `.feature` e cucumber no crate correspondente; IPC e permissões também verificados no Windows.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` nos crates envolvidos e cenários Vitest verdes.
- [ ] Gate 6: comandos finos; domain sem deps externas de produção; application só depende de domain; boundaries frontend verdes.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios comprovados, inclusive falhas/compensação; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada; nenhuma sessão sem App ID deixa o jogo modificado.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
