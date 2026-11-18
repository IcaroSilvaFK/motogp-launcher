---
title: "Persista dados JSON e trate erros sem perda"
labels: [feature, domain, application, infrastructure, interface]
depends_on: ["01"]
branch: feat/02-persistir-dados-e-erros
---

## Contexto

Adapta a Task 2. Configuração, biblioteca, perfis e manifesto precisam de persistência que suporte caminhos Windows com espaços e acentos e nunca substitua dados corrompidos por defaults silenciosos. As estruturas do plano com derives serde e erros de I/O não podem migrar integralmente para domain/application.

## Escopo (por camada)

- **Domain:** defina `Config { game_dir, active_profile }` como modelo puro com valores opcionais e default, sem serde ou erros de bibliotecas externas. Use tipos padrão de caminho quando necessários, sem acessar o disco.
- **Application:** introduza o port de configuração e os erros de aplicação necessários. Eles expressam falhas sem depender de `std::io::Error`, `serde_json`, ZIP ou Tauri. Casos de uso dependem de ports tipados, não de um store JSON concreto.
- **Infrastructure:** implemente `DataDir` e os caminhos `config.json`, `profiles.json`, `library.json`, `deployed.json` e `library/`. Implemente `load_json` e `save_json` e os DTOs serde; converta DTOs para modelos puros. Ausência retorna default, JSON existente inválido retorna erro. Salve em `<path>.tmp`, crie pais e substitua o destino atomicamente no Windows, inclusive quando o arquivo já existe; uma falha não pode destruir a versão anterior. Use pretty-print e limpe temporários após sucesso.
- **Interface:** reserve a resolução de `app_data_dir` do Tauri para o bootstrap da issue 08. Mapeie erros de aplicação/adapter para erro serializável em português na fronteira Tauri. O antigo `AppError` que encapsula I/O/JSON/ZIP fica fora do núcleo; não imponha sua representação aos ports.
- Reutilize o adapter de armazenamento nas próximas issues sem antecipar as entidades de biblioteca/perfil/deploy.

## Fora de escopo

Regras de perfis, importação e deploy. Migração de schemas e sincronização remota. Nunca tratar arquivo de zero bytes como se não existisse: apenas ausência é default; `[]` válido poderá representar coleção vazia na issue 04.

## Critérios de aceite (Gherkin)

Arquivo: `crates/infrastructure/tests/features/persistencia.feature`, executado com `cucumber` e diretórios temporários. Cenários de aplicação usam repositório fake, sem fs/Tauri/rede.

```gherkin
# language: pt
Funcionalidade: Persistir dados locais com segurança
  Cenário: Ler configuração ausente
    Dado que config.json ainda não existe
    Quando carrego a configuração
    Então recebo a configuração padrão

  Cenário: Salvar e ler em caminho com espaços e acentos
    Dado um diretório chamado "Usuário Padrão"
    E uma configuração com game_dir em "Program Files (x86)" e active_profile "p1"
    Quando salvo e leio config.json
    Então os valores retornam sem alteração
    E os diretórios pais são criados
    E não resta arquivo config.json.tmp

  Cenário: Recusar JSON corrompido
    Dado um arquivo existente contendo "{ not json"
    Quando tento carregar seus dados
    Então recebo um erro de leitura em português
    E o arquivo original permanece intacto
    E nenhum default é salvo por cima dele

  Cenário: Substituir dados existentes com segurança no Windows
    Dado que config.json contém uma configuração válida
    Quando salvo uma segunda configuração
    Então a próxima leitura retorna a segunda configuração completa
    E não há arquivo temporário pendente

  Cenário: Preservar a versão anterior quando a escrita falha
    Dado um arquivo válido já salvo
    Quando uma falha de I/O impede a substituição atômica
    Então a operação retorna erro
    E o arquivo anterior continua íntegro e legível
```

## Testes TDD (escrever antes)

Preserve os três nomes do plano em **crates/infrastructure**, no módulo de store; são testes do adapter com `tempfile`, não testes do domínio:

- `missing_file_returns_default`: ausência resulta em `Config::default()`.
- `roundtrip_in_path_with_accents`: roundtrip completo e ausência de `.tmp` no caminho acentuado.
- `corrupted_json_is_an_error`: erro sem sobrescrita dos bytes originais.
- Acrescente `save_replaces_existing_file_atomically` e `failed_save_preserves_previous_file` para verificar o contrato no Windows.
- **Application:** `config_repository_error_is_propagated`, com fake do port; **domain:** defaults de `Config` testados sem I/O.

## Definition of Done

- [ ] Red → Green → Refactor registrado; testes antes do código e nomes originais preservados.
- [ ] Cenários implementados em `.feature`/runner cucumber e rastreados aos testes; fixtures só em diretórios temporários.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: `cargo test -p motogp-infrastructure --test features`, demais runners (`cargo test --workspace --test features`) e cenários Vitest verdes.
- [ ] Gate 6: domain sem deps externas de produção; application só depende de domain; nenhuma regra importa fs, serde ou Tauri; boundaries frontend verdes.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` demonstram ≥90% domain/application por camada/crate e ≥80% geral por stack, conforme issue 01.
- [ ] Gate 8: todos os critérios comprovados; checks **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes do PR; falhas de persistência não causam perda silenciosa.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
