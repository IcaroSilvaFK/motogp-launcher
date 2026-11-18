# MotoGP Mod Launcher — Design

## Intent

A study project: a lightweight mod launcher for the MotoGP games (Milestone, Unreal Engine) built from scratch, inspired by SwoftyLauncher (F1 25), but without the engine code-injection part. The author wants to understand each piece; the Swofty code (Rust) serves as reading reference, not as a base.

## Goals (MVP)

1. Locate the game folder (auto-detection via Steam + manual choice).
2. Import `.pak` mods (with UE5 `.utoc`/`.ucas`, if any), loose or inside `.zip`, into a launcher library.
3. Profiles: ordered lists of mods (e.g. "Vanilla", "Pinturas 2025").
4. Play: deploys the profile's mods into `~mods`, opens the game through Steam, and when the game closes removes what it deployed.

## Non-goals

- Code injection/hooking into the engine (Swofty's "Mixin").
- Database editor, in-game UI, online mod browser.
- 7z/RAR, Linux/Proton, auto-update.

## Anti-goals (a failure even if it works)

- Deleting any file in `~mods` that the launcher did not put there.
- Leaving the game modified after a session (including after a launcher crash).
- A heavy executable/installer (target: < 15 MB).

## Constraints

- Windows 10/11, Steam. Development and execution on Windows (not WSL).
- Tauri 2 + stable Rust; React + TypeScript + Vite frontend; pnpm.
- Do not copy Swofty code (PolyForm Noncommercial license).

## Approaches considered

| Option | Pros | Cons |
|---|---|---|
| **Tauri (Rust + web)** — chosen | Same language as Swofty (study), ~5–10 MB binary | Rust learning curve |
| Wails (Go + web) | Author already knows Go, quick to deliver | Learns less; does not mirror Swofty |
| C# + Avalonia | Native UI, author knows .NET | Heavier, styling is laborious |

Electron discarded (≈150 MB, against "lightweight"). The user chose Tauri.

## How decisions (Claude's calls, correctable)

- **React+TS frontend**: the author already uses React/TS.
- **Game detection**: reads `SteamPath` from the registry (`HKCU\Software\Valve\Steam`), walks `libraryfolders.vdf`, looks in `steamapps/common` for folders whose name starts with `MotoGP`. The App ID comes from the `appmanifest_*.acf` whose `installdir` matches the folder. No App ID is hard-coded (works for 24, 25…).
- **Mods folder**: `<root>/<Project>/Content/Paks/~mods`, where `<Project>` is the root subfolder that contains `Content/Paks`.
- **Grouping**: files with the same base name (`X.pak`, `X.utoc`, `X.ucas`) form a mod; `.pak` is mandatory.
- **Load order**: profile order; files deployed as `NNN_<modId>_<originalName>.<ext>` (the original `_P` suffix stays at the end).
- **Deployment**: hard link on the same volume, otherwise copy. Everything deployed goes into `deployed.json`; cleanup deletes only what is there. When the launcher opens, if `deployed.json` is not empty, it cleans (crash recovery).
- **End of session**: poll every 2 s for processes whose executable is inside the game root; if it never appears within 120 s, or disappears for 2 polls in a row, clean up.
- **Data**: Tauri's `app_data_dir` (`%APPDATA%\<identifier>`): `config.json`, `profiles.json`, `library.json`, `library/<modId>/…`, `deployed.json`.
- **Mod id**: first 12 hex characters of the SHA-256 of the `.pak` content → reimporting the same mod does not duplicate it.

## Left to the builder

Visual look of the interface, exact error messages, React component structure.
