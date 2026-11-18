---
title: "Detecte instalações MotoGP pela Steam"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/05-detectar-instalacoes-do-jogo
---

## Contexto

Adapta a Task 5. A instalação pode estar em qualquer biblioteca Steam. O App ID vem do manifesto correspondente, sem constante para MotoGP 24/25 ou outra edição. A escolha manual deve funcionar mesmo sem Steam detectada.

## Escopo (por camada)

- **Domain:** defina `GameInfo { name, root, mods_dir, app_id }` puro, com `app_id` opcional. Mantenha as regras de seleção MotoGP, correspondência de `installdir` sem distinguir maiúsculas e exclusão da pasta Engine livres de fs/registro/serde.
- **Application:** implemente detectar instalações e inspecionar uma raiz usando ports de localização Steam e leitura da estrutura do jogo. Faça a seleção e validação, sem chamar `winreg` ou fs diretamente. Falta de Steam retorna lista vazia; uma raiz sem `Content/Paks` retorna "pasta não parece um jogo Unreal: Content/Paks não encontrado". A inspeção não cria `~mods`.
- **Infrastructure:** implemente `parse_library_folders`, `parse_app_manifest`, descoberta física de `Content/Paks`, leitura de manifestos e adapter Windows para `HKCU\Software\Valve\Steam`, valor `SteamPath`. Parsers VDF/ACF pertencem ao adapter do formato Steam, mesmo sendo funções puras. Desescape barras em `libraryfolders.vdf`; procure pastas em `steamapps/common` com prefixo MotoGP sem distinguir maiúsculas. Busque o App ID nos `appmanifest_*.acf` de `steamapps`, cujo `installdir` corresponde à pasta raiz. Ignore `Engine` na busca da primeira subpasta imediata com `Content/Paks`.
- Derive `mods_dir` como `<raiz>/<Projeto>/Content/Paks/~mods`, sem fixar o nome do projeto. Mantenha os adapters Windows condicionados à plataforma sem levar dependências Windows para o núcleo.

## Fora de escopo

App ID fixo, iniciar Steam/jogo, criar `~mods`, interface de configurações e suporte a Linux/Proton.

## Critérios de aceite (Gherkin)

Arquivos: `crates/application/tests/features/deteccao-jogo.feature` com ports fake e `crates/infrastructure/tests/features/steam.feature` com árvore Steam temporária; runners `cucumber`. O registro real é smoke separado no Windows, sem requisito para testes automatizados.

```gherkin
# language: pt
Funcionalidade: Encontrar a instalação correta do MotoGP
  Cenário: Interpretar bibliotecas Steam com barras escapadas
    Dado um libraryfolders.vdf com "C:\\Program Files (x86)\\Steam" e "D:\\SteamLibrary" escapados no formato VDF
    Quando leio as bibliotecas
    Então recebo os dois caminhos Windows com barras desescapadas
    E os espaços são preservados

  Cenário: Ler o App ID e o diretório de instalação
    Dado um manifesto ACF com appid "123" e installdir "MotoGP25"
    Quando interpreto o manifesto
    Então recebo o ID 123 e o nome "MotoGP25"

  Cenário: Inspecionar um jogo Unreal válido
    Dado "common/MotoGP25/MotoGP25/Content/Paks"
    E um appmanifest em steamapps com installdir "motogp25" e appid "123"
    Quando inspeciono a raiz "common/MotoGP25"
    Então mods_dir termina em "MotoGP25/Content/Paks/~mods"
    E o App ID é 123
    E a pasta ~mods não é criada

  Cenário: Rejeitar raiz sem Paks do projeto
    Dado uma raiz sem Content/Paks em suas subpastas de projeto
    Quando a inspeciono
    Então recebo erro contendo "Content/Paks não encontrado"

  Cenário: Detectar somente MotoGP
    Dado bibliotecas com um MotoGP25 válido e um OtherGame válido
    Quando detecto os jogos
    Então somente MotoGP25 aparece

  Cenário: Ignorar a pasta Engine
    Dado uma raiz com Engine/Content/Paks e Projeto/Content/Paks
    Quando procuro os pacotes do jogo
    Então uso Projeto/Content/Paks

  Cenário: Permitir pasta manual sem manifesto Steam
    Dado uma raiz Unreal válida sem manifesto correspondente
    Quando a inspeciono
    Então recebo GameInfo com App ID ausente
    E nenhuma pasta de mods é criada

  Cenário: Não encontrar Steam no registro
    Dado que SteamPath não está disponível
    Quando detecto instalações automaticamente
    Então recebo uma lista vazia
    E a inspeção manual continua disponível
```

## Testes TDD (escrever antes)

Preserve os cinco nomes do plano:

- **Infrastructure/Steam**, funções de parsing puras: `parse_library_folders_unescapes_backslashes`, `parse_app_manifest_reads_id_and_installdir`.
- **Infrastructure**, inspeção com árvore em `tempfile`: `inspect_game_finds_mods_dir_and_app_id`, `inspect_game_without_paks_errors`, `detect_games_only_returns_motogp_folders`. Exercite os casos de uso com adapters, mantendo a regra de seleção no núcleo.
- **Application**, unitários com fakes: `missing_steam_returns_empty_list`, `manual_game_without_manifest_has_no_app_id`, `engine_folder_is_not_selected` e `manifest_match_is_case_insensitive`.
- **Infrastructure:** `inspection_does_not_create_mods_directory`, garantindo ausência de efeitos de escrita.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado, mantendo cinco nomes originais e fixtures independentes da Steam instalada.
- [ ] Todos os cenários têm `.feature` e runner cucumber; smoke do registro Windows registrado separadamente.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features` e cenários Vitest verdes.
- [ ] Gate 6: dependências Cargo para dentro e boundaries verdes; winreg/fs/parsers Steam ficam fora do núcleo.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios comprovados e nenhum App ID fixo; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes do PR.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
