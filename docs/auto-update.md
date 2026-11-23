# In-app auto-update (Tauri Updater)

The installed launcher checks whether a newer version exists, downloads the package, installs it and restarts, all from the UI.

## Flow

1. A merge into `main` (production) triggers the pipeline, which produces the installer and the GitHub Release `v<version>` together with `latest.json` (version, notes and signature).
2. On startup, the app queries `https://github.com/<owner>/<repo>/releases/latest/download/latest.json`.
3. If the version is greater than the installed one, the front end shows "New version available". The user clicks **Update**, and the app downloads, verifies the signature, installs and restarts.

## Setup (once)

1. **Generate the key pair** (the private key **never** goes into the repo):
   ```sh
   pnpm tauri signer generate -w ~/.tauri/motogp-launcher.key
   ```
2. **GitHub secrets** (Settings → Secrets → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY`: contents of the `.key` file
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the chosen password
3. **Plugin:** `pnpm tauri add updater` and `pnpm add @tauri-apps/plugin-process`.
4. **`src-tauri/tauri.conf.json`:**
   ```json
   {
     "bundle": { "createUpdaterArtifacts": true },
     "plugins": {
       "updater": {
         "pubkey": "<contents of .key.pub>",
         "endpoints": ["https://github.com/<owner>/<repo>/releases/latest/download/latest.json"],
         "windows": { "installMode": "passive" }
       }
     }
   }
   ```
5. **Capabilities** (`src-tauri/capabilities/default.json`): add `"updater:default"` and `"process:allow-restart"`.

**Current key (done 2026-11-23):** the production pair was generated on the maintainer's PC at `~/.tauri/motogp-launcher.key` (password in `~/.tauri/motogp-launcher.key.password`); both GitHub secrets are set and the public key is in `tauri.conf.json`. Back up the key and the password outside this machine: if either is lost, installed apps can no longer verify new updates and users must reinstall manually.

## Clean Architecture on the front end

- `src/application/ports/UpdateService.ts`: port with `check()`, `downloadAndInstall(onProgress)` and `restart()`.
- `src/infrastructure/tauri/TauriUpdateService.ts`: adapter over `@tauri-apps/plugin-updater` and `@tauri-apps/plugin-process` (`createTauriUpdateService()`).
- `src/application/useCases/checkForUpdate.ts`: use case `createUpdateController`, tested with `FakeUpdateService` (TDD).
- `src/ui/UpdateBanner.tsx`: presentation only (new version, notes, progress, error, button).
- `src/main.tsx`: composition root. It injects the Tauri service, wires `isSessionClean` from the launcher state (`selectSessionClean`) and triggers one non-blocking check on startup.

**Session guard:** the update is shown even during a game session, but the button is disabled and `install()` itself refuses (`session` error) while a session is `starting`/`running` or recovery is `game-running`/`failed`, so `~mods` is never cleaned under a running game.

**Feed:** the endpoint is `https://github.com/IcaroSilvaFK/motogp-launcher/releases/latest/download/latest.json`. The repository `IcaroSilvaFK/motogp-launcher` is public, so the app needs no token.


## Base BDD scenario

```gherkin
Feature: Automatic update
  Scenario: A newer version exists
    Given the installed app is on version "1.0.0"
    And the latest release is "1.1.0"
    When the app starts
    Then the banner "New version 1.1.0 available" is shown
    When the user clicks "Update"
    Then the download progress is shown
    And the app restarts on version "1.1.0"

  Scenario: Already on the latest version
    Given the installed app is on version "1.1.0"
    And the latest release is "1.1.0"
    When the app starts
    Then no update banner is shown
```

> Private repo: the `latest.json` of a private release is not reachable without a token. For auto-update to work, the repo (or just its releases) must be public, or another host must be used for `latest.json`.
