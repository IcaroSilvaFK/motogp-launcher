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

## Clean Architecture on the front end

- `application/ports/UpdateService.ts`: interface `check()`, `downloadAndInstall(onProgress)`, `restart()`.
- `infrastructure/tauri/TauriUpdateService.ts`: adapter using `@tauri-apps/plugin-updater` and `plugin-process`.
- `application/useCases/checkForUpdate.ts`: use case, tested with a fake of the port (TDD).
- `ui/UpdateBanner.tsx`: presentation only (new version, notes, progress bar, button).

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
