---
title: "Valide o build Windows e a sessão real"
labels: [chore, interface, frontend, ci]
depends_on: ["10"]
branch: feat/11-validar-build-e-jogo-real
---

## Contexto

Adapta a Task 10 original, agora depois do auto-update. Valide o produto integrado no Windows 10/11 nativo: executável leve, instaladores, mods no jogo, preservação de arquivos externos, recuperação de crash e atualização assinada. Testes com fakes não substituem evidência de jogo e instalação reais.

## Escopo (por camada)

- **Interface/build:** execute `pnpm tauri build` e valide executável release, setup NSIS `.exe` e `.msi`. Como agora existe workspace Cargo na raiz, o destino padrão é `target/release/` e `target/release/bundle/`, e não obrigatoriamente `src-tauri/target/`; respeite `CARGO_TARGET_DIR` se configurado. Comprove o caminho efetivo dos artefatos e meça o executável release **<15 MB**.
- **CI:** confirme **Rust core** e **Frontend** antes de empacotar, artefatos de `dev/qas` retidos 30 dias e release `v<versão>`/`latest.json` em `main`. Verifique o tamanho dos instaladores separadamente do executável e registre a composição, inclusive eventual bootstrap/runtime WebView2. O plano fornece limite objetivo para o executável; a meta do design de instalador leve também deve ser avaliada e qualquer excesso justificado, sem omitir tamanho.
- **Application/infrastructure integradas:** detecte MotoGP real com App ID do manifesto, importe um mod válido, escolha perfil e jogue. Confira a ordem/prefixos dos arquivos, a mudança no jogo e limpeza após duas ausências no poll de 2 s (aproximadamente 4 s, com tolerância de agendamento registrada).
- **Segurança:** mantenha um `.pak` manual de controle em `~mods` durante a sessão; compare conteúdo antes/depois. Teste crash matando o launcher, feche o jogo e reabra o launcher; a recuperação remove somente seus próprios arquivos. Não encerre o jogo do usuário sem combinar a sessão de teste; utilize instalação/ambiente de validação.
- **Frontend/updater integrados:** valide atualização de uma versão instalada para uma release posterior assinada, mostrando notas/progresso e reiniciando na nova versão. Confirme ausência de banner quando atual e preservação de biblioteca, perfis, configuração e arquivo manual. Use releases/host de teste controlados, sem publicar versões artificiais em produção apenas para testar.
- **Evidências:** registre versões do Windows/jogo/app, caminhos, tamanhos, saídas dos gates e resultado por cenário em `docs/validacao/` na implementação desta issue, sem dados pessoais/chaves. Ausência de jogo, acesso à release ou chave real é pendência, não aprovação tácita.

## Fora de escopo

Novas funcionalidades, Linux/Proton, copiar código Swofty, injeção na engine e refactors não necessários para corrigir falhas encontradas. Não declarar teste real aprovado com base apenas em mocks ou build compilado.

## Critérios de aceite (Gherkin)

Arquivo: `src-tauri/tests/features/release-windows.feature`, runner `cucumber`. Automatize inspeção dos artefatos e contratos em fixtures. Identifique os cenários de jogo/instalação com tags `@windows @jogo_real` / `@updater_real` e execute-os explicitamente no ambiente Windows de validação com coleta de evidência; o CI comum continua executando a suíte determinística sem exigir jogo licenciado. Os cenários reais são testes obrigatórios para concluir esta issue, ainda que fora do runner comum do CI. Reflexos de UI também têm cenários Vitest conforme issues 09/10.

```gherkin
# language: pt
Funcionalidade: Entregar uma versão Windows validada
  Cenário: Gerar executável e instaladores
    Dado o workspace completo com gates aprovados
    Quando executo pnpm tauri build no Windows
    Então encontro o executável release e os instaladores NSIS e MSI no target efetivo
    E o executável tem menos de 15 MB
    E os tamanhos dos instaladores ficam registrados

  @windows @jogo_real
  Cenário: Jogar com um mod real e limpar no término
    Dado um MotoGP Steam instalado detectado com App ID
    E um mod válido importado e selecionado em um perfil
    Quando clico em Jogar
    Então o jogo apresenta o mod
    E os arquivos aplicados têm prefixos ordenados iniciando em "000_"
    Quando fecho o jogo
    Então em aproximadamente 4 segundos de polls a interface anuncia a sessão encerrada
    E somente os arquivos aplicados pelo launcher são removidos

  @windows @jogo_real
  Cenário: Preservar um pak manual durante a sessão
    Dado um pak manual em ~mods com conteúdo registrado antes do teste
    Quando executo e encerro uma sessão com mods do launcher
    Então o pak manual permanece com o mesmo conteúdo

  @windows @jogo_real
  Cenário: Recuperar após encerramento forçado do launcher
    Dado uma sessão com mods aplicados e um pak manual de controle
    Quando encerro o launcher pelo Gerenciador de Tarefas
    E fecho o jogo e reabro o launcher
    Então os arquivos da sessão interrompida são removidos pela recuperação
    E o pak manual permanece intacto
    E uma nova sessão pode ser iniciada

  @windows @updater_real
  Cenário: Atualizar uma instalação preservando dados
    Dado uma versão instalada com biblioteca, perfis e configuração preenchidos
    E uma versão posterior assinada no feed controlado
    Quando inicio o launcher e clico em Atualizar sem sessão ativa
    Então vejo notas e progresso
    E após instalar o launcher reinicia na versão posterior
    E biblioteca, perfis, configuração e arquivos manuais permanecem íntegros
    Quando verifico novamente o mesmo feed
    Então não vejo banner de atualização

  Cenário: Respeitar a distribuição por ambiente
    Dado os relatórios dos pipelines das branches de ambiente
    Quando confiro seus artefatos e condições de publicação
    Então os instaladores só foram gerados após os gates verdes
    E dev e qas retêm artefatos por 30 dias
    E somente main publica a release da versão configurada com latest.json contendo as assinaturas dos pacotes
```

## Testes TDD (escrever antes)

O plano não nomeava testes desta etapa. Antes de acrescentar lógica de validação ou corrigir falhas:

- **Interface/build**, verificações assertivas de artefatos: `release_executable_is_below_15_mb`, `windows_bundles_include_nsis_and_msi` e `workspace_artifact_directory_is_resolved`. Use fixtures para o caminho de falha e artefatos reais para a prova final.
- **CI**, contrato da configuração: `installer_build_depends_on_validation`, `non_production_artifacts_have_30_day_retention` e `production_release_manifest_contains_artifact_signatures`.
- **Application**, confirme as regressões já existentes `clean_removes_only_deployed_files`, `deploy_cleans_previous_session_first`, `deploy_refuses_to_overwrite_foreign_file`, `play_without_app_id_does_not_deploy` e `startup_recovers_previous_deployment`, nas camadas indicadas nas issues de origem; não duplique testes só para aumentar contagem.
- **Frontend/application**, confirme `restart_only_after_successful_install` e `active_or_unclean_session_blocks_install` da issue 10.
- Qualquer bug encontrado exige primeiro um teste de regressão Red na camada responsável, depois a correção mínima e Refactor. Validações de jogo/atualização reais são testes de aceitação, não unitários; execute os cenários marcados e anexe as evidências.

## Definition of Done

- [ ] Red → Green → Refactor demonstrado para alterações/correções; nenhuma correção de produção sem regressão correspondente.
- [ ] `.feature` executado com cucumber; todos os cenários reais concluídos e evidenciados no Windows, sem confundir exclusão do CI com dispensa de validação.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: runners `cargo test --workspace --test features`, cenários Vitest e execução explícita dos cenários reais verdes.
- [ ] Gate 6: deps Cargo e boundaries frontend verdes; alterações de build não introduzem regras no Tauri/UI.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios rastreados e evidências de build/jogo/crash/arquivo externo/updater registradas; **Rust core** e **Frontend** verdes no CI.
- [ ] Executável <15 MB; NSIS/MSI gerados; tamanhos/limitações dos instaladores documentados e avaliados pelo QA.
- [ ] QA Haiku aprovado e revisão Codex aplicada; pendência de hardware, jogo, feed ou assinatura impede declarar esta issue concluída.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; promoção apenas `dev → qas → main`, sem commit direto nessas branches.
