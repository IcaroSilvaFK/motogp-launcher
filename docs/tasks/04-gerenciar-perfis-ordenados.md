---
title: "Gerencie perfis e a ordem dos mods"
labels: [feature, domain, application, infrastructure]
depends_on: ["02"]
branch: feat/04-gerenciar-perfis-ordenados
---

## Contexto

Adapta a Task 4 e antecipa a proteção do perfil Vanilla exigida pela Interface original. A ordem de `mod_ids` é a ordem de carga; o backend deve preservá-la sem depender de componentes React.

## Escopo (por camada)

- **Domain:** defina `Profile { id, name, mod_ids }`, nome não vazio após trim e proteção contra exclusão do ID `vanilla`. Preserve a ordem informada dos mods. Defina erros de perfil ausente/nome vazio em tipos puros.
- **Application:** implemente listar, criar, renomear, excluir, buscar, definir mods e remover um mod de todos os perfis por meio de `ProfileRepository`. Introduza fonte de IDs/tempo substituível nos testes; mantenha IDs novos no formato `p<unix_millis>` sem colisão quando houver criações próximas. Não use relógio real nos testes do núcleo.
- **Inicialização:** na ausência do arquivo ou coleção JSON válida vazia, crie e persista `Profile { id: "vanilla", name: "Vanilla", mod_ids: [] }`. Arquivo vazio em bytes/JSON inválido continua sendo erro da issue 02. Liste Vanilla antes de criar o primeiro perfil para não perdê-lo.
- **Infrastructure:** implemente repositório JSON em `profiles.json`, DTOs serde e fonte real dos IDs; reutilize persistência e erros da issue 02.
- Retorne "nome do perfil vazio" e "perfil não encontrado" nos respectivos casos. A remoção de referências será consumida pela issue 08 após remover um mod da biblioteca.

## Fora de escopo

UI de perfis, deploy, sincronização entre máquinas e validação da existência dos mods contra a biblioteca. A resolução de IDs faltantes ocorre na issue 08; esta issue pode seguir em paralelo à biblioteca e à detecção.

## Critérios de aceite (Gherkin)

Arquivos: `crates/application/tests/features/perfis.feature` com fakes e `crates/infrastructure/tests/features/persistencia-perfis.feature` para roundtrip; runners `cucumber`.

```gherkin
# language: pt
Funcionalidade: Manter perfis ordenados
  Cenário: Inicializar perfis ausentes ou coleção vazia
    Dado que não há arquivo de perfis ou ele contém uma coleção JSON vazia válida
    Quando listo os perfis
    Então recebo somente Vanilla com ID "vanilla" e nenhum mod
    E esse perfil fica persistido

  Cenário: Criar e listar perfil
    Dado que Vanilla já existe
    Quando crio "Pinturas 2025"
    Então a lista contém dois perfis
    E o novo perfil tem nome "Pinturas 2025" e ID próprio

  Cenário: Preservar a ordem de mods
    Dado um perfil existente
    Quando defino seus mods como "b", "a", "c"
    Então a leitura retorna exatamente essa ordem

  Cenário: Excluir perfil comum
    Dado um perfil diferente de Vanilla
    Quando o excluo
    Então ele deixa de aparecer na lista persistida

  Cenário: Remover referências em todos os perfis
    Dado que vários perfis contêm o mod "a"
    Quando removo as referências ao mod "a"
    Então nenhum perfil contém "a"
    E a ordem dos outros mods é preservada

  Cenário: Rejeitar nome em branco
    Dado um nome composto apenas por espaços
    Quando tento criar ou renomear um perfil com esse nome
    Então recebo erro "nome do perfil vazio"
    E os dados anteriores são preservados

  Cenário: Proteger Vanilla no backend
    Dado o perfil com ID "vanilla"
    Quando tento excluí-lo pelo caso de uso
    Então a exclusão é recusada
    E Vanilla continua salvo

  Cenário: Renomear perfil e tratar ID inexistente
    Dado um perfil existente
    Quando o renomeio para "Corrida"
    Então o nome atualizado é persistido
    Quando busco um ID inexistente
    Então recebo erro "perfil não encontrado"
```

## Testes TDD (escrever antes)

Preserve os seis nomes do plano em **crates/application**, casos de uso com repositório/fonte de IDs fake:

- `empty_store_seeds_vanilla`.
- `create_then_list`.
- `set_mods_preserves_order`.
- `delete_removes_profile`.
- `remove_mod_everywhere_strips_from_all_profiles`.
- `blank_name_is_rejected`.
- **Domain/application:** acrescente `vanilla_cannot_be_deleted`, `rename_persists_new_name`, `unknown_profile_returns_error` e `profile_ids_do_not_collide` para as regras adicionais.
- **Infrastructure:** acrescente `profiles_roundtrip_preserves_order` e `invalid_profiles_json_is_not_reseeded`, com `tempfile`. Não mova esses testes de I/O para o domínio.

## Definition of Done

- [ ] Red → Green → Refactor demonstrado; seis nomes originais mantidos e proteção Vanilla testada no backend.
- [ ] Cenários em `.feature` executados com cucumber e mapeados aos testes correspondentes.
- [ ] Gate 1: `cargo fmt --all --check` e `pnpm prettier --check .` verdes.
- [ ] Gate 2: `cargo clippy --workspace --all-targets -- -D warnings` e `pnpm lint` verdes.
- [ ] Gates 3–4: `pnpm tsc --noEmit`, `cargo test --workspace` e `pnpm test` verdes.
- [ ] Gate 5: runners `cargo test --workspace --test features` e cenários Vitest verdes.
- [ ] Gate 6: dependências Cargo para dentro e boundaries verdes; nenhuma leitura de JSON/relógio real em testes de application/domain.
- [ ] Gate 7: `cargo llvm-cov` e `pnpm test --coverage` com ≥90% domain/application por camada/crate e ≥80% geral por stack.
- [ ] Gate 8: critérios comprovados; **Rust core** e **Frontend** verdes no CI.
- [ ] QA Haiku aprovado e revisão Codex aplicada antes do PR.
- [ ] Branch a partir de `dev`; PR para `dev` com `Closes #<nº>` real; sem commit direto em `dev/qas/main`.
