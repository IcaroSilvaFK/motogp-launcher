# motogp-launcher

## Memória e preferências

Sempre salvar memórias, decisões e preferências **localmente no projeto** (neste `CLAUDE.md` ou em `docs/`), nunca só na memória global do usuário.

## Fluxo de desenvolvimento (multi-agente)

Todo trabalho de desenvolvimento segue este pipeline de papéis:

1. **Agente especializado geral (o chat)** — ponto de contato com o usuário; entende o pedido, orquestra o fluxo e consolida o resultado.
2. **Codex** — organiza e elabora as tasks (quebra o trabalho em tarefas detalhadas).
3. **Agente PO (1)** — coordena os desenvolvedores: distribui as tasks e acompanha a entrega.
4. **Agentes desenvolvedores (N)** — implementam as tasks, em paralelo quando possível. **Modelo: Sonnet** (`model: "sonnet"`).
5. **Agentes de QA (N)** — testam e validam o que os devs entregaram. **Modelo: Haiku** (`model: "haiku"`).
6. **Codex** — revisão final e melhorias do que foi implementado.

Regra: o chat não implementa direto. Demanda de desenvolvimento → Codex gera as tasks → PO dispara os devs (Sonnet) → QA valida (Haiku) → Codex revisa → chat reporta ao usuário.
