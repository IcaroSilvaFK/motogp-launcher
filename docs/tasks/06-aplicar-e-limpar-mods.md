---
title: "Aplique e limpe mods com manifesto seguro"
labels: [feature, domain, application, infrastructure]
depends_on: ["03"]
branch: feat/06-aplicar-e-limpar-mods
---

## Contexto

Adapta a Task 6. O manifesto `deployed.json` é a fonte da recuperação de sessão; arquivos do usuário nunca podem ser sobrescritos ou apagados. A limpeza após crash acontece na próxima abertura do launcher, conforme o design, e será ligada ao bootstrap na issue 08.

## Escopo (por camada)

- **Domain:** defina `Deployed { mods_dir, files }` e `deployed_name(index, mod_id, original)`, puros. O índice é a posição do mod, começando em zero, e é compartilhado pelos arquivos do mesmo mod. Nome: `NNN_<modId>_<nomeOriginal>`, preservando o `_P` original antes da extensão.
- **Application:** implemente aplicar e limpar com ports de filesystem e repositório de manifesto. Antes de um novo deploy, limpe a sessão anterior; se a limpeza falhar, não prossiga. Perfil vazio não cria pasta nem arquivos novos. Para cada arquivo, recuse destino existente e use criação exclusiva, sem intervalo que permita sobrescrita concorrente. Atualize o manifesto a cada arquivo aplicado, nunca apenas no fim.
- **Infrastructure:** implemente hard link e fallback para cópia, inclusive entre volumes. O fallback também deve recusar sobrescrita. Implemente remoção apenas dos caminhos registrados e seguros, tolerando `NotFound`. Ao concluir a limpeza, persista `Deployed::default()` e devolva a contagem efetivamente removida; em falha parcial, retenha os itens ainda pendentes para nova tentativa.
- **Segurança e recuperação:** valide nomes relativos simples e confinamento ao diretório autorizado, inclusive manifesto lido do disco; não aceite `..`/caminhos absolutos. Registre e teste o protocolo entre materialização e persistência para não deixar arquivo aplicado sem rastreio recuperável em interrupção. A sequência antiga "criar e só depois salvar" tem uma janela de crash e, sozinha, não comprova essa garantia; não resolva isso apagando por prefixo ou incluindo arquivo alheio no manifesto. Use journal/registro de propriedade recuperável, com testes de interrupção nos pontos críticos.
- Reutilize biblioteca e persistência das issues 03/02; `deploy` consome entradas já ordenadas, sem conhecer UI ou perfis concretos.

## Fora de escopo

Apagar `~mods` inteiro, varrer/apagar por prefixo, substituir arquivos externos, watcher/processos e limpeza enquanto o launcher está morto. Não prometer limpeza imediata após crash sem a reabertura prevista no design.

## Critérios de aceite (Gherkin)

Arquivos: `crates/domain/tests/features/nomes-deploy.feature`, `crates/application/tests/features/deploy.feature` com fakes e falhas injetadas, `crates/infrastructure/tests/features/deploy-arquivos.feature` com fs temporário; runners `cucumber`.

```gherkin
# language: pt
Funcionalidade: Aplicar somente os mods da sessão e recuperar sua limpeza
  Cenário: Aplicar na ordem do perfil
    Dado os mods B e A nessa ordem
    Quando aplico seus arquivos
    Então os destinos são "000_<idB>_B.pak" e "001_<idA>_A.pak"
    E cada arquivo aplicado tem registro persistido recuperável

  Cenário: Preservar o sufixo de patch
    Dado um arquivo original "Ducati_P.pak"
    Quando calculo seu destino
    Então o nome termina em "_Ducati_P.pak"

  Cenário: Limpar somente o que foi aplicado
    Dado arquivos aplicados registrados e o arquivo manual "meu_mod.pak" em ~mods
    Quando limpo a sessão
    Então somente os arquivos registrados são removidos
    E meu_mod.pak mantém os mesmos bytes
    E o manifesto fica vazio após a limpeza completa

  Cenário: Tolerar arquivo aplicado já ausente
    Dado que um arquivo registrado já foi removido
    Quando limpo a sessão
    Então a limpeza termina sem erro por esse arquivo
    E a contagem considera apenas arquivos efetivamente removidos

  Cenário: Limpar a sessão anterior antes de reaplicar
    Dado uma sessão anterior com A aplicado
    Quando aplico somente B
    Então os arquivos de A são removidos antes dos de B
    E apenas B fica registrado

  Cenário: Recusar colisão com arquivo do usuário
    Dado um arquivo manual no nome exato do destino planejado
    Quando tento aplicar o mod por hard link ou fallback de cópia
    Então recebo erro contendo "já existe"
    E o arquivo manual fica intacto e fora da limpeza

  Cenário: Aplicar perfil vazio sem criar diretório
    Dado que não existe sessão anterior nem pasta ~mods
    Quando aplico um perfil sem mods
    Então ~mods continua ausente

  Cenário: Recuperar interrupção durante a aplicação
    Dado uma interrupção em um ponto entre registro e materialização de um arquivo
    Quando retomo a recuperação do manifesto e do journal
    Então todo arquivo comprovadamente aplicado pode ser limpo
    E nenhum arquivo alheio é apagado

  Cenário: Retentar limpeza parcialmente falha
    Dado dois arquivos registrados e uma falha ao remover o segundo
    Quando limpo a sessão
    Então recebo um erro
    E o segundo arquivo continua rastreado para recuperação

  Cenário: Rejeitar caminho fora do diretório autorizado
    Dado um manifesto adulterado com um caminho contendo ".."
    Quando tento limpar a sessão
    Então a operação retorna erro
    E nenhum arquivo externo é removido
```

## Testes TDD (escrever antes)

Preserve os sete nomes do plano:

- **Domain**, unitário: `deploy_preserves_p_suffix`.
- **Application**, unitários com fs/manifesto fake: `deploy_writes_files_in_profile_order_with_prefix`, `deploy_cleans_previous_session_first`, `deploy_empty_profile_leaves_dir_untouched`.
- **Infrastructure**, contratos com fs temporário: `clean_removes_only_deployed_files`, `clean_tolerates_already_missing_file`, `deploy_refuses_to_overwrite_foreign_file`.
- **Application:** acrescente `interrupted_deploy_remains_recoverable`, com interrupções entre registro/materialização, `failed_clean_keeps_pending_entries` e `invalid_manifest_path_is_rejected`.
- **Infrastructure:** acrescente `copy_fallback_never_overwrites_existing_file` e `hard_link_failure_falls_back_to_copy`, usando falha controlada do adapter para não exigir dois volumes no CI. Cubra também o caminho de hard link bem-sucedido no Windows.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado; sete nomes originais preservados; falhas críticas testadas antes da implementação.
- [ ] Cenários em `.feature`/cucumber nos crates corretos; recuperação prova propriedade, sem heurística por nome/prefixo.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` e cenários Vitest verdes.
- [ ] Gate 6: dependências Cargo para dentro e boundaries verdes; regras sem I/O e adapters sem decidir ordem de perfil.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios e falhas de disco/interrupção comprovados; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada; arquivos alheios nunca sobrescritos/apagados e pendências não descartadas.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
