---
title: "Atualize o aplicativo pela interface"
labels: [feature, frontend, infrastructure, interface, ci]
depends_on: ["09"]
branch: feat/10-atualizar-aplicativo-pelo-front
---

## Contexto

Issue extra exigida por `docs/auto-update.md`, posicionada depois da Interface e antes da validação de release. A documentação e o pedido atual passam a incluir auto-update, superando o não objetivo do design antigo. O app consulta releases de produção ao abrir e só instala após ação do usuário.

## Escopo (por camada)

- **Frontend/domain:** defina apenas dados puros necessários para versão, notas e progresso, sem tipos do SDK updater.
- **Frontend/application:** crie `ports/UpdateService.ts` com `check()`, `downloadAndInstall(onProgress)` e `restart()`, e `useCases/checkForUpdate.ts`. Coordene checagem ao abrir, download, instalação e reinício por meio do port. Use a semântica de versão do updater, sem comparar strings lexicograficamente. Ausência de atualização não exibe banner; erros são recuperáveis e não bloqueiam o restante do launcher.
- **Frontend/infrastructure:** implemente `tauri/TauriUpdateService.ts` com `@tauri-apps/plugin-updater` e `@tauri-apps/plugin-process`, preservando o objeto de atualização validado retornado pela checagem. Propague início/progresso/término, falhas de rede, assinatura e instalação. Progresso sem tamanho total conhecido pode ser indeterminado.
- **Frontend/ui:** crie `UpdateBanner.tsx` apenas de apresentação, sob `src/ui`, com versão, notas em formato seguro, barra de progresso e botão Atualizar acessíveis. Evite dupla instalação; reinicie somente após instalação concluída. Desabilite a instalação enquanto uma sessão está iniciando/ativa ou a limpeza está pendente, para não encerrar o watcher com mods aplicados; revalide essa condição no caso de uso, não só no botão.
- **Interface/configuração Tauri:** registre os plugins Rust updater e process, dependências JS correspondentes e capabilities `updater:default` e `process:allow-restart`. Configure `bundle.createUpdaterArtifacts = true`, chave **pública**, endpoint real `https://github.com/<owner>/<repo>/releases/latest/download/latest.json` e Windows `installMode: "passive"`. O bootstrap injeta o serviço; domain/application Rust permanecem sem dependências do updater.
- **CI/release:** adapte o workflow existente, mantendo os gates antes do build. Use versão do `tauri.conf.json`, NSIS/MSI assinados para atualização e release `v<versão>` com `latest.json` em `main`. `dev/qas` produzem artefatos por 30 dias, sem publicar release de produção. Preserve o comportamento já existente em vez de duplicar jobs.
- **Setup externo necessário para concluir esta issue:** gerar o par de chaves em local seguro fora do repositório; cadastrar `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` como secrets do GitHub; versionar somente a pública. Identifique owner/repo e confirme acesso ao feed sem token embutido. Se o repositório é privado, providencie distribuição pública dos artefatos/manifesto ou host apropriado, sem presumir acesso anônimo à release privada. A issue não pode ser encerrada com endpoint/chave fictícios ou assinatura desativada. Registre somente evidências não secretas.

## Fora de escopo

Atualizar mods ou o jogo, instalação silenciosa sem clique, token GitHub no app, canais beta e alteração dos gates de cobertura. Não versionar chave privada nem expor segredo em logs. A execução deste planejamento não gera chaves, altera secrets ou publica releases.

## Critérios de aceite (Gherkin)

Fonte: `tests/features/frontend/auto-update.feature`; cenários frontend em Vitest + Testing Library, especialmente `src/application/useCases/__tests__/checkForUpdate.test.ts` e `src/ui/__tests__/UpdateBanner.test.tsx`. Contratos Windows de assinatura/configuração: `src-tauri/tests/features/updater.feature`, runner `cucumber` com fixtures locais. Gates automatizados usam fakes/fixtures; a atualização real com duas versões é comprovada na issue 11.

```gherkin
# language: pt
Funcionalidade: Atualizar o launcher instalado pela interface
  Cenário: Encontrar e instalar versão mais nova
    Dado que o app instalado está na versão "1.0.0"
    E o serviço informa a release "1.1.0" com notas e assinatura válida
    Quando o app inicia
    Então vejo "Nova versão 1.1.0 disponível" e as notas
    Quando clico em Atualizar sem sessão de jogo pendente
    Então vejo o progresso do download
    E a instalação é concluída antes do pedido de reinício
    E o app reinicia na versão "1.1.0"

  Cenário: Já estar na última versão
    Dado que o app instalado e a última release estão na versão "1.1.0"
    Quando o app inicia
    Então nenhum banner de atualização é exibido
    E nenhum download ou reinício é solicitado

  Cenário: Não instalar versão anterior
    Dado o app na versão "1.10.0" e um feed com "1.9.0"
    Quando verifico atualizações
    Então a release anterior não é oferecida para instalação

  Cenário: Preservar uso do launcher sem conexão
    Dado que a consulta do feed falha por rede
    Quando o app inicia
    Então vejo um aviso recuperável em português
    E continuo podendo usar biblioteca, perfis e configurações
    E nenhum reinício é solicitado

  Cenário: Recusar pacote com assinatura inválida
    Dado uma atualização cujo pacote não passa na verificação de assinatura
    Quando tento atualizá-lo
    Então a instalação é recusada e o erro fica visível
    E o app não reinicia nem substitui a versão instalada

  Cenário: Tratar falha na instalação
    Dado que o download termina mas a instalação falha
    Quando o serviço retorna o erro
    Então o erro fica visível e a tentativa deixa de estar em progresso
    E nenhum reinício é solicitado

  Cenário: Evitar instalações simultâneas
    Dado uma atualização em andamento
    Quando aciono Atualizar novamente
    Então não há segundo download nem segunda instalação

  Cenário: Proteger uma sessão de jogo
    Dado uma sessão iniciando, rodando ou aguardando limpeza
    Quando uma atualização fica disponível
    Então posso ver a nova versão
    Mas a instalação e o reinício ficam indisponíveis até a sessão estar limpa

  Cenário: Publicar feed somente pelo fluxo de produção
    Dado um push em uma branch de ambiente com gates aprovados
    Quando o pipeline gera os instaladores
    Então dev e qas guardam NSIS e MSI por 30 dias sem release de produção
    E main publica a release da versão configurada com latest.json e artefatos assinados
    E a configuração do aplicativo contém somente a chave pública
```

## Testes TDD (escrever antes)

Não há nomes originais nesta issue extra. Escreva primeiro:

- **Frontend/application**, fake de `UpdateService`: `checkForUpdate_returns_new_release`, `checkForUpdate_returns_none_when_current`, `older_release_is_not_offered`, `check_failure_does_not_block_launcher`, `download_progress_is_forwarded`, `restart_only_after_successful_install`, `install_failure_does_not_restart`, `concurrent_update_is_rejected` e `active_or_unclean_session_blocks_install`.
- **Frontend/ui**, Testing Library: `shows_new_version_notes_and_update_button`, `hides_banner_when_up_to_date`, `shows_determinate_or_indeterminate_progress`, `update_error_is_accessible` e `session_state_disables_update_action`.
- **Frontend/infrastructure**, SDK mockado na fronteira: `updater_adapter_checks_downloads_installs_and_restarts`, `invalid_signature_error_is_propagated` e `version_check_uses_updater_semantics`; recusa real de pacote inválido também exige teste de contrato, sem simular sucesso do plugin.
- **Interface/CI:** `updater_config_contains_public_key_and_endpoint`, `updater_capabilities_are_minimal`, `unsigned_or_tampered_update_is_rejected` com fixture local de assinatura e `release_job_requires_successful_validation`. Verifique condições/artefatos do workflow existente sem publicar em testes.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado; cenários frontend implementados como describe/it, assinatura/configuração cobertas no runner cucumber.
- [ ] Setup de chave pública, secrets e endpoint real concluído com evidência sem segredos; acesso ao feed comprovado no contexto do app instalado.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` e cenários Vitest verdes, sem depender de feed público mutável.
- [ ] Gate 6: boundaries/deps Cargo verdes; SDK updater/process só nos adapters e bootstrap, sem import direto pela UI.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios rastreados aos testes; **Rust core** e **Frontend** verdes; build/release condicionado aos gates.
- [ ] QA Haiku aprovado e revisão Codex aplicada; validação integrada da atualização real fica explicitamente na issue 11, sem declarar que um fake prova reinício na nova versão.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; promoções `dev → qas → main`, sem commit direto nessas branches.
