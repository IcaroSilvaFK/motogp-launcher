---
title: "Inicie o jogo e acompanhe a sessão"
labels: [feature, domain, application, infrastructure]
depends_on: ["05"]
branch: feat/07-iniciar-e-acompanhar-sessao
---

## Contexto

Adapta a Task 7. Separe a máquina de estados da observação real de processos e da abertura de URLs. Esta issue prepara os componentes de sessão; a orquestração com deploy e os comandos Tauri entram na issue 08.

## Escopo (por camada)

- **Domain:** implemente `launch_url`, `WatchEvent { Started, Ended, NeverStarted }`, `WatchState` e `watch_step` como regras puras. Com App ID, gere `steam://rungameid/<id>`; sem ele, retorne "App ID do jogo não encontrado; abra o jogo pela Steam uma vez". Primeira observação positiva gera Started; duas ausências consecutivas depois disso geram Ended; observação positiva zera a contagem. Nunca observado até 120 s inclusive gera NeverStarted. Eventos terminais não se repetem.
- **Application:** defina ports de abertura Steam, inspeção de processos e tempo/poll conforme o caso de uso precisa. O acompanhamento usa intervalo de 2 s (`POLL`), recebendo observações dos adapters. Testes usam relógio e processos fake, sem sleeps reais de 120 s.
- **Infrastructure:** implemente inspeção `sysinfo` e relógio real. Considere um processo do jogo somente se seu executável estiver dentro da raiz do jogo, sem distinguir maiúsculas no Windows e respeitando componentes do caminho: `MotoGP25Backup` não está dentro de `MotoGP25`. Executável indisponível não comprova jogo rodando. A abertura real via plugin opener será conectada no bootstrap da issue 08; mantenha aqui o port e o contrato da URL.
- Retorne eventos de aplicação sem serialização Tauri; serialização e emissão para a UI ficam na interface.

## Fora de escopo

Deploy/clean, comandos Tauri, UI, App ID fixo, detecção por título de janela e polling no frontend.

## Critérios de aceite (Gherkin)

Arquivos: `crates/domain/tests/features/estado-sessao.feature`, `crates/application/tests/features/acompanhamento-sessao.feature` e `crates/infrastructure/tests/features/processos.feature`; runners `cucumber`. Use listas de processos e tempo controlados nos testes, sem iniciar jogo real.

```gherkin
# language: pt
Funcionalidade: Identificar início e término da sessão
  Cenário: Montar URL com App ID detectado
    Dado um jogo com App ID 123
    Quando preparo sua abertura
    Então recebo "steam://rungameid/123"

  Cenário: Recusar abertura sem App ID
    Dado um jogo sem App ID
    Quando preparo sua abertura
    Então recebo "App ID do jogo não encontrado; abra o jogo pela Steam uma vez"

  Cenário: Encerrar após duas ausências consecutivas
    Dado que o primeiro poll encontra o jogo e emite Started
    Quando o próximo poll não encontra o processo
    Então nenhum término é emitido
    Quando o poll seguinte também não encontra o processo
    Então Ended é emitido uma única vez

  Cenário: Zerar ausências quando o processo reaparece
    Dado a sequência de polls presente, ausente, presente, ausente
    Quando avanço a máquina de estados
    Então Started é emitido na primeira presença
    E Ended não é emitido

  Cenário: Informar jogo que nunca iniciou
    Dado que nenhum poll encontrou o processo
    Quando o tempo decorrido chega a 119 segundos
    Então nenhum término é emitido
    Quando o tempo chega a 120 segundos
    Então NeverStarted é emitido uma única vez

  Cenário: Observar somente executáveis dentro da raiz
    Dado uma raiz de jogo "C:/Jogos/MotoGP25"
    E executáveis em "c:/jogos/motogp25/Binaries/jogo.exe" e "C:/Jogos/MotoGP25Backup/outro.exe"
    Quando classifico os processos
    Então apenas o primeiro é considerado processo do jogo

  Cenário: Usar o intervalo de acompanhamento definido
    Dado um relógio e um inspetor de processos controlados
    Quando acompanho uma sessão
    Então as consultas são agendadas em intervalos de 2 segundos
```

## Testes TDD (escrever antes)

Preserve os cinco nomes do plano em **crates/domain**, sem Tauri/sysinfo/tempo real:

- `launch_url_uses_app_id`.
- `launch_url_without_app_id_errors`.
- `started_then_ended_after_two_missing_polls`.
- `flapping_resets_missing_counter`.
- `never_started_after_120s`.
- **Domain:** acrescente `terminal_watch_event_is_not_repeated`.
- **Application:** acrescente `watcher_polls_every_two_seconds` com relógio/inspector fake.
- **Infrastructure:** acrescente `process_path_comparison_is_case_insensitive`, `sibling_directory_is_not_game_root` e `missing_process_executable_is_ignored`, usando dados controlados para o adapter, sem depender dos processos da máquina do CI.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado; cinco nomes originais mantidos; nenhum teste do núcleo depende de processos reais/sleep.
- [ ] Cada cenário tem `.feature` e teste cucumber no crate indicado.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` e cenários Vitest verdes.
- [ ] Gate 6: dependências Cargo para dentro e boundaries verdes; sysinfo/opener/Tauri fora do núcleo.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: limites temporais e de caminho comprovados; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes do PR.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
