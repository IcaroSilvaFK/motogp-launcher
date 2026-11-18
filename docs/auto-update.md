# Auto-update pelo front (Tauri Updater)

O launcher instalado verifica se existe versão nova, baixa o pacote, instala e reinicia, tudo pela interface.

## Fluxo

1. Um merge em `PRD` dispara o pipeline, que gera o instalador e a GitHub Release `v<versão>` junto com o `latest.json` (versão, notas e assinatura).
2. Ao abrir, o app consulta `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.
3. Se a versão for maior que a instalada, o front mostra "Nova versão disponível". O usuário clica em **Atualizar**, o app baixa, confere a assinatura, instala e reinicia.

## Setup (uma vez)

1. **Gerar o par de chaves** (a chave privada **nunca** vai para o repo):
   ```sh
   pnpm tauri signer generate -w ~/.tauri/motogp-launcher.key
   ```
2. **Secrets no GitHub** (Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY`: conteúdo do arquivo `.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: a senha escolhida
3. **Plugin:** `pnpm tauri add updater` e `pnpm add @tauri-apps/plugin-process`.
4. **`src-tauri/tauri.conf.json`:**
   ```json
   {
     "bundle": { "createUpdaterArtifacts": true },
     "plugins": {
       "updater": {
         "pubkey": "<conteúdo do .key.pub>",
         "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"],
         "windows": { "installMode": "passive" }
       }
     }
   }
   ```
5. **Capabilities** (`src-tauri/capabilities/default.json`): adicionar `"updater:default"` e `"process:allow-restart"`.

## Arquitetura Limpa no front

- `application/ports/UpdateService.ts`: interface `check()`, `downloadAndInstall(onProgress)`, `restart()`.
- `infrastructure/tauri/TauriUpdateService.ts`: adapter que usa `@tauri-apps/plugin-updater` e `plugin-process`.
- `application/useCases/checkForUpdate.ts`: caso de uso, testado com um fake do port (TDD).
- `ui/UpdateBanner.tsx`: só apresentação (versão nova, notas, barra de progresso, botão).

## Cenário BDD base

```gherkin
Funcionalidade: Atualização automática
  Cenário: Existe versão mais nova
    Dado que o app instalado está na versão "1.0.0"
    E a última release é "1.1.0"
    Quando o app inicia
    Então o banner "Nova versão 1.1.0 disponível" é exibido
    Quando o usuário clica em "Atualizar"
    Então o progresso do download é exibido
    E o app reinicia na versão "1.1.0"

  Cenário: Já está na última versão
    Dado que o app instalado está na versão "1.1.0"
    E a última release é "1.1.0"
    Quando o app inicia
    Então nenhum banner de atualização é exibido
```

> Repo privado: o `latest.json` de release privada não é acessível sem token. Para o auto-update funcionar, o repo (ou só as releases) precisa ser público, ou é preciso usar outro host para o `latest.json`.
