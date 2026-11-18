---
title: "Importe mods para a biblioteca local"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/03-importar-biblioteca-de-mods
---

## Contexto

Adapta a Task 3. Importe arquivos `.pak` com irmãos `.utoc/.ucas`, ou ZIPs aninhados, para `library/<modId>/`. A biblioteca é independente da pasta do jogo e da sessão.

## Escopo (por camada)

- **Domain:** defina `ModEntry { id, name, files, size_bytes }` puro e a regra `group_pak_files`. Agrupe pelo nome-base, reconheça extensões sem distinguir maiúsculas, ignore extensões estranhas e exija `.pak` em cada grupo. O nome de exibição remove apenas o `_P` final; nomes de arquivo mantêm o sufixo.
- **Application:** implemente listar, importar e remover usando ports `ModRepository`, leitura/cópia de arquivos, leitura de arquivo compactado e hash do conteúdo, introduzidos conforme necessário. Deduplicate pelo ID de 12 hex minúsculos do SHA-256 do `.pak`. Importar `.pak/.utoc/.ucas` considera apenas irmãos de mesmo nome-base; importar ZIP retorna todos os grupos válidos. Retorne a entrada existente em reimportação. Teste decisões com fakes, sem fs ou ZIP real no núcleo.
- **Infrastructure:** implemente adapters de fs, índice JSON com o store da issue 02, SHA-256 em streaming e ZIP. Extraia para temporário dentro da biblioteca, achate subpastas e ignore entradas sem `enclosed_name()` seguro. Não escreva fora da biblioteca; rejeite colisões de nomes ao achatar subpastas, sem sobrescrever arquivos. ZIP sem `.pak` retorna erro; extensão não suportada retorna erro em português. Registre apenas importações concluídas e limpe temporários.
- **Remoção:** apague a pasta do ID solicitado e sua entrada do índice com tratamento de erro. Valide IDs/caminhos na fronteira de arquivos. A coordenação com referências em perfis será feita na issue 08, usando a operação da issue 04.

## Fora de escopo

RAR/7z, download de mods, edição do conteúdo, UI, cópia para `~mods` e atualização do aplicativo.

## Critérios de aceite (Gherkin)

Arquivos: `crates/domain/tests/features/agrupamento-mods.feature` para regras puras, `crates/application/tests/features/biblioteca.feature` com fakes e `crates/infrastructure/tests/features/importacao-arquivos.feature` com fixtures fs/ZIP; runners `cucumber`.

```gherkin
# language: pt
Funcionalidade: Organizar mods importados na biblioteca
  Cenário: Importar um pak solto
    Dado um arquivo "A.pak"
    Quando o importo
    Então recebo uma entrada com o arquivo "A.pak"
    E seu ID tem 12 caracteres hexadecimais minúsculos derivados do SHA-256 do pak
    E library.json e library/<id>/A.pak contêm o mod importado

  Cenário: Buscar apenas os irmãos do mod escolhido
    Dado "A_P.pak", "A_P.utoc", "A_P.ucas" e "B.pak" na mesma pasta
    Quando importo "A_P.pak"
    Então o mod tem nome "A" e os três arquivos de A_P
    E B não é importado

  Cenário: Encontrar mods em subpastas de um ZIP
    Dado um ZIP contendo "x/y/A.pak" e "B.pak"
    Quando o importo
    Então recebo duas entradas com nomes de arquivo achatados

  Cenário: Rejeitar ZIP sem pak
    Dado um ZIP contendo apenas "readme.txt"
    Quando o importo
    Então recebo erro contendo "nenhum .pak"

  Cenário: Rejeitar grupo sem pak
    Dado um grupo contendo apenas "A.utoc"
    Quando valido o agrupamento
    Então recebo erro contendo "não tem arquivo .pak"

  Cenário: Reimportar sem duplicar
    Dado que um pak já foi importado
    Quando importo novamente o mesmo conteúdo
    Então recebo a entrada existente
    E a biblioteca continua com uma entrada para esse ID

  Cenário: Remover um mod
    Dado um mod importado
    Quando removo seu ID
    Então sua pasta e entrada do índice deixam de existir

  Cenário: Ignorar travessia de diretórios no ZIP
    Dado um ZIP contendo "../evil.pak" e "ok.pak"
    Quando o importo
    Então somente "ok.pak" entra na biblioteca
    E nenhum arquivo é escrito fora do diretório controlado

  Cenário: Rejeitar colisão ao achatar subpastas
    Dado um ZIP contendo "x/A.pak" e "y/A.pak" com conteúdos distintos
    Quando o importo
    Então recebo um erro de colisão
    E nenhum arquivo existente da biblioteca é sobrescrito

  Cenário: Rejeitar formato não suportado
    Dado um arquivo "mod.rar"
    Quando tento importá-lo
    Então recebo erro contendo "formato não suportado"
```

## Testes TDD (escrever antes)

Mantenha os oito nomes do plano. Separe políticas unitárias de provas dos adapters:

- **Application**, casos de uso com fakes: `reimport_same_pak_does_not_duplicate`.
- **Domain**, agrupamento puro: `utoc_without_pak_errors`; acrescente `group_accepts_case_insensitive_extensions` e `group_ignores_unrelated_extensions`.
- **Infrastructure**, integração com fs/ZIP temporários: `import_single_pak_copies_into_library`, `import_pak_picks_up_utoc_ucas_siblings`, `import_zip_with_nested_folders_finds_all_paks`, `import_zip_without_pak_errors`, `remove_deletes_dir_and_index_entry`, `zip_entry_with_parent_traversal_is_ignored`. Exercite os casos de uso com adapters reais, sem levar I/O para application.
- **Application:** acrescente `unsupported_extension_is_rejected` e `failed_import_does_not_publish_index_entry` com fakes.
- **Infrastructure:** acrescente `flattened_zip_name_collision_is_rejected` e `mod_id_matches_first_twelve_sha256_hex` com conteúdo e digest esperados conhecidos.

## Definition of Done

- [ ] Red → Green → Refactor evidenciado; oito nomes originais preservados e novos comportamentos cobertos antes do código.
- [ ] Cada cenário tem `.feature` e teste cucumber no crate indicado; fakes no núcleo e fs/ZIP só em infrastructure.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test --workspace --test features`, incluindo os três crates, e cenários Vitest verdes.
- [ ] Gate 6: deps Cargo para dentro e boundaries verdes; domínio sem serde/hash/ZIP/fs, aplicação só depende de domain em produção.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: todos os critérios rastreados; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada; entradas maliciosas não escapam da biblioteca.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
