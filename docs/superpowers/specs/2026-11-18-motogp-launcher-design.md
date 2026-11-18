# MotoGP Mod Launcher — Design

## Intenção

Projeto de estudo: um launcher de mods para os jogos MotoGP (Milestone, Unreal Engine) feito do zero, leve, inspirado no SwoftyLauncher (F1 25), mas sem a parte de injeção de código na engine. O autor quer entender cada peça; o código do Swofty (Rust) serve de referência de leitura, não de base.

## Objetivos (MVP)

1. Localizar a pasta do jogo (auto-detecção via Steam + escolha manual).
2. Importar mods `.pak` (com `.utoc`/`.ucas` do UE5, se houver) soltos ou dentro de `.zip` para uma biblioteca do launcher.
3. Perfis: listas ordenadas de mods (ex.: "Vanilla", "Pinturas 2025").
4. Jogar: aplica os mods do perfil em `~mods`, abre o jogo pela Steam, e ao fechar o jogo remove o que aplicou.

## Não-objetivos

- Injeção/hook de código na engine (o "Mixin" do Swofty).
- Editor de banco de dados, UI in-game, navegador de mods online.
- 7z/RAR, Linux/Proton, auto-update.

## Anti-objetivos (falha mesmo funcionando)

- Apagar qualquer arquivo em `~mods` que o launcher não colocou lá.
- Deixar o jogo modificado depois de uma sessão (incluindo após crash do launcher).
- Executável/instalador pesado (alvo: < 15 MB).

## Restrições

- Windows 10/11, Steam. Desenvolvimento e execução no Windows (não WSL).
- Tauri 2 + Rust estável; frontend React + TypeScript + Vite; pnpm.
- Não copiar código do Swofty (licença PolyForm Noncommercial).

## Abordagens consideradas

| Opção | Prós | Contras |
|---|---|---|
| **Tauri (Rust + web)** — escolhida | Mesma linguagem do Swofty (estudo), binário ~5–10 MB | Curva do Rust |
| Wails (Go + web) | Autor já sabe Go, rápido de entregar | Aprende menos; não espelha o Swofty |
| C# + Avalonia | UI nativa, autor conhece .NET | Mais pesado, estilização trabalhosa |

Electron descartado (≈150 MB, contra o "leve"). O usuário escolheu Tauri.

## Decisões de como (chamadas do Claude, corrigíveis)

- **Frontend React+TS**: o autor já usa React/TS.
- **Detecção do jogo**: lê `SteamPath` no registro (`HKCU\Software\Valve\Steam`), percorre `libraryfolders.vdf`, procura em `steamapps/common` pastas cujo nome começa com `MotoGP`. O App ID vem do `appmanifest_*.acf` cujo `installdir` bate com a pasta. Nenhum App ID fica fixo no código (funciona para 24, 25…).
- **Pasta de mods**: `<raiz>/<Projeto>/Content/Paks/~mods`, onde `<Projeto>` é a subpasta da raiz que contém `Content/Paks`.
- **Agrupamento**: arquivos com o mesmo nome-base (`X.pak`, `X.utoc`, `X.ucas`) formam um mod; `.pak` é obrigatório.
- **Ordem de carga**: ordem do perfil; arquivos aplicados como `NNN_<modId>_<nomeOriginal>.<ext>` (o sufixo `_P` original continua no fim).
- **Aplicação**: hard link quando mesmo volume, senão cópia. Tudo que foi aplicado vai para `deployed.json`; a limpeza apaga só o que está lá. Na abertura do launcher, se `deployed.json` não estiver vazio, limpa (recuperação de crash).
- **Fim da sessão**: poll a cada 2 s de processos cujo executável está dentro da raiz do jogo; se nunca aparecer em 120 s, ou sumir por 2 polls seguidos, limpa.
- **Dados**: `app_data_dir` do Tauri (`%APPDATA%\<identifier>`): `config.json`, `profiles.json`, `library.json`, `library/<modId>/…`, `deployed.json`.
- **Id do mod**: 12 primeiros hex do SHA-256 do conteúdo do `.pak` → reimportar o mesmo mod não duplica.

## Deixado para o builder

Visual da interface, mensagens de erro exatas, estrutura de componentes React.
