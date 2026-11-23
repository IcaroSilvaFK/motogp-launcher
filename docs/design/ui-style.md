# UI style: graphite dark (direction v3)

## Decisions

- 2026-11-23 (issue #48): the user chose a dark launcher UI.
- 2026-11-23, after the first version (a racing-style skin): "less playful, more elaborate/polished and easier to use". The direction became a **professional desktop tool**: neutral graphite surfaces, crisp 1px borders, dense readable rows, one restrained accent.
- 2026-11-23, v3: the user pointed at a third-party PC mod launcher as the level of polish to reach. Its **patterns** were adopted (top icon+label tabs, a persistent bottom status bar with the game and Play, rail + content layouts, facet filters with counts, settings as a section nav plus cards of rows, small outlined secondary buttons). Its brand, colors, copy and every feature we do not have (online catalog, leaderboards, donations, social login, notifications, storage usage, mod versions, conflicts) were **not** copied.

Rules:

- Pure CSS in `src/ui/styles.css`; no UI framework, no icon library (icons are inline SVG in `Icon.tsx`, one stroke style: 20x20 viewBox, 1.5 stroke, round caps and joins), no remote fonts.
- System font stack `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`.
- Dark only (`color-scheme: dark`; `index.html` sets the same body background as `--bg` to avoid a flash).
- Colors, spacing, radii, type sizes, motion and the shell dimensions are tokens in the `:root` block; rules reference them. Only component-local sizes (hairlines, 2px indicator bars, tile and icon sizes, column minimums) are written in place.
- No `clip-path`, no italic or uppercase display type, radii between 4px and 8px (enforced by the style feature test).
- **One accent, used sparingly:** a warm racing red (`--accent`) fills only **Play** (and the update action inside its banner). It also draws the current-tab underline, the current facet/section/profile bar, the brand mark and icon tiles (`--accent-text`). A screen's own main action (Import on the empty library, Save in the profile editor) uses the accent **outline** (`btn-accent-outline`) so it never competes with Play.
- Destructive actions are outline buttons with danger text, never filled. In dense lists (library rows) they stay neutral until the row is hovered or the button is focused.
- Minimum supported window is **900x600** (`src-tauri/tauri.conf.json`). Every screen fits without horizontal scroll: a 208px rail, a fluid content column capped at 1180px, ellipsis on long names.
- `prefers-reduced-motion: reduce` disables every animation and transition (pulse dots, skeletons, hovers).

The domain has no mod category, version or per-mod enabled flag. "Enabled" means "in the active profile's `mod_ids`". Since #54 each library row has a **switch** bound to that: turning it on or off calls `controller.toggleModInActiveProfile(id)`, which saves the active profile (new mods go last in the load order) and sets no success notice (the row is the feedback; a failure shows the usual error notice). The facet counts, the subtitle and the current facet's list follow from the reloaded profiles. Search and facets filter on the client as UI-local state (`LibraryScreen`).

- 2026-11-23 (issue #54): the row switch replaced the read-only profile badge in the same slot. It is a `<button role="switch" aria-checked>` named "Include <mod> in <profile>", with the state written next to it ("In active profile" / "Not in active profile"; the text sits outside the button so the accessible name stays short and label-in-name is not broken). When it cannot be used it gets `aria-disabled="true"` (not `disabled`, so it stays focusable and its `title` tooltip works in WebView2), a `title`, and `aria-describedby` pointing to one visible hint above the list: locked while a session is starting or running (`selectProfileLocked`), Vanilla active, or no active profile. While the controller is busy it is inert without a hint. The checked track uses `--running` (the same green as the running dot), not the accent, so the accent stays reserved for Play.

## Layout

```
┌ topbar (48px): brand ─────── [Library] [Profiles] [Settings] ───────────────┐
│ app-messages: stale-session notice, error/success notice, update banner     │
│ main (scrolls): page head (h2 title + live subtitle, toolbar actions)       │
│                 rail (208px) │ content                                       │
└ statusbar (60px): game tile + name + store │ session status │ profile ▾ PLAY ┘
```

- **Top bar** (`<header>`, role banner): brand (`<h1>`) and the main navigation (`TopNav`, `<nav aria-label={t.nav.label}>`), icon + label tabs with `aria-current="page"` and a 2px accent underline.
- **Status bar** (`StatusBar`, `<footer>`, role contentinfo), always visible:
  - left: game tile, game name (tooltip = root folder) and a `Steam` store badge when an App ID exists; without a game, "No game selected" plus a **Set up game** link-button that opens Settings; then the launcher version (`statusbar-version`, `v0.1.0`, 12px muted caption after a 1px hairline, tooltip "Launcher version"). The caption is not rendered while the version is unknown or could not be read (issue #53);
  - middle: `SessionStatus` (`role="status"`, announced) or, when nothing is happening and Play is possible, a plain "Ready to play" line (not announced);
  - right: the active-profile select (`#active-profile`, visible label) and **Play** (`btn btn-primary play`, 40px, filled play glyph).
- **Page head** on every screen: one visible `<h2>` (the screen title, also the `aria-labelledby` target) and a `page-subtitle` with live counts (`2 mods · 1 in Mine`, `2 profiles · active: Mine`, a short description for Settings). Toolbar actions sit on the right.
- **Library:** filter rail (search, `Filter` group of facet buttons with counts: All / In active profile / Not in active profile, `aria-pressed`; the drop hint as a dashed drop zone) + mod list (tile with the initial, name, size · file count, active-profile switch with its state label, Remove). No results shows the message with **Clear filters**.
- **Profiles:** rail with "Your profiles" (each row: name button with `aria-pressed`, mod count, `Active` badge) and the "New profile" form; content shows the editor (rename, mods checklist and load order side by side, Save) or an empty state asking to pick a profile.
- **Settings:** section nav (`<nav aria-label="Settings sections">`, anchor links with `aria-current="location"`) + sections of **cards of rows** (`setting-row`: label + help text on the left, control on the right). Game: folder row (Detect games, Choose folder…), the current game details, detected games. Preferences: Language. About (always last, `AboutSection`): a "Launcher version" row (value on the right, "Unknown" muted when it cannot be read) and, only when an updater exists, an "Updates" row with a small outlined **Check for updates** button.
- **Manual update check (Settings > About, issue #53):** user-initiated (`updater.check()` without `background`). While it runs the button is disabled and reads "Checking…". The result sits under the row in a polite live region (`role="status"`): "The launcher is up to date." with a green check glyph (only after a user check in this app session, because a failed silent startup check also ends in `none`; the flag lives in `AppInfoContext` so it survives leaving Settings), "Version X is available" (semibold) plus **Install update** (`btn btn-sm btn-accent-outline`: Play keeps the only fill; disabled with the session hint as its `aria-describedby` while a session is not clean), the downloading / restarting lines, or the translated `t.update.errors[kind]` in danger text (never the raw message). The update banner keeps its own behaviour; both read the same controller state.
- **Notices and banners** sit between the top bar and the content, aligned to the content column.
- **Loading:** while `status === 'loading'`, `<main aria-busy="true">` shows "Loading…" and skeleton rows instead of a screen, so no empty state flashes.

## Tokens (`:root` in `styles.css`)

| Token                                         | Value                             | Purpose                                                |
| --------------------------------------------- | --------------------------------- | ------------------------------------------------------ |
| `--bg`                                        | `#0f1115`                         | Page background, inputs, inset lists                   |
| `--surface-1`                                 | `#15181d`                         | Top bar, status bar, cards, panels, mod list           |
| `--surface-2`                                 | `#1b1f25`                         | Hover, current facet/section                           |
| `--surface-3`                                 | `#232830`                         | Button hover, tiles, selected profile, counts          |
| `--border` / `--border-strong`                | `#262b33` / `#363c46`             | Separators / control and button borders                |
| `--text` / `--text-muted`                     | `#e7e9ec` / `#9aa2ad`             | Body / secondary text                                  |
| `--accent` / `--accent-hover`                 | `#d23a1e` / `#b52f17`             | Play fill, indicator bars                              |
| `--on-accent`                                 | `#ffffff`                         | Label on the accent                                    |
| `--accent-text`                               | `#ff8a66`                         | Accent on dark surfaces (outline buttons, tiles, link) |
| `--accent-tint`                               | `#2a1611`                         | Accent-outline button hover                            |
| `--focus-ring`                                | `#5aa9ff`                         | `:focus-visible` outline (blue, not red)               |
| `--danger-text/border/bg`                     | `#ff8f86` / `#6b2f2c` / `#2a1716` | Destructive outline buttons                            |
| `--info/success/warning/error-bg/fg/border`   | see file                          | Banners and badges                                     |
| `--running`                                   | `#3ddc84`                         | Running / ready dot                                    |
| `--space-1..6`                                | 4, 8, 12, 16, 24, 32px            | Spacing (4/8px grid)                                   |
| `--topbar-height` / `--statusbar-height`      | 48px / 60px                       | Shell rows                                             |
| `--rail-width` / `--content-max`              | 208px / 1180px                    | Rail column / content cap                              |
| `--control-height(-sm)` / `--play-height`     | 32px (28px) / 40px                | Inputs and buttons / Play                              |
| `--radius-sm/md/lg`                           | 4px / 6px / 8px                   | Corner radii                                           |
| `--fs-title/section/body/small/caption`       | 20 / 15 / 14 / 13 / 12px          | Type scale                                             |
| `--font-body`, `--font-mono`                  | system stacks                     | Text / paths and code                                  |
| `--transition`, `--pulse`                     | 120ms ease, 1.2s alternate        | Hover motion, status dot and skeleton pulse            |

Type scale: page title 20px semibold, section and group titles 15px semibold, body 14px, subtitles 13px, captions 12px (hints, meta lines, badges, counts). Sizes and counts use `font-variant-numeric: tabular-nums` (`.num`).

## Contrast pairs checked

`src/ui/__tests__/ui-style.feature.test.tsx` parses `styles.css` and enforces the WCAG relative-luminance ratios:

- `--text` and `--text-muted` on `--bg`, `--surface-1`, `--surface-2`, `--surface-3`: at least 4.5 (muted is 5.75 on the lightest).
- `--accent-text` on the same four levels: at least 4.5 (6.40 on the lightest).
- `--on-accent` on `--accent` (4.81) and `--accent-hover` (6.21): at least 4.5.
- `--focus-ring` on `--bg` and `--surface-1`: at least 3.

Banner pairs (`--*-fg` on `--*-bg`, all above 10) and the danger text on the surfaces (8.07) were checked by hand. Row switch (#54, no new tokens): the on track `--running` against the row surfaces and the `--bg` thumb on it are far above the 3:1 non-text minimum; the off thumb `--text-muted` on the `--surface-3` track is 5.75; the state label uses `--text-muted` / `--success-fg`, already above 4.5 on the surfaces.

## Components

- Shell: `app-shell`, `topbar`, `brand`, `top-nav`, `nav-tab`, `app-messages`, `statusbar` (`statusbar-game`, `game-tile`, `statusbar-game-name`, `statusbar-version`, `statusbar-message`, `statusbar-idle`, `statusbar-actions`), `profile-field`.
- Page: `page`, `page-head`, `page-heading`, `page-subtitle`, `page-actions`, `split`/`library-layout`, `split-rail`, `split-main`, `rail-title`, `panel`.
- Buttons: `btn` (small outlined secondary), `btn-primary` (Play, update), `btn-accent-outline` (a screen's main action), `btn-ghost`, `btn-danger` (outline), `btn-sm`, `btn-icon`, `btn-link`; Play is `btn btn-primary play`.
- Forms: `input`, `field` (inline label), `form-field` (stacked label, input), `form-row`, `section-title`, `hint`.
- Filters: `search`, `facet-group`, `facet` (`aria-pressed`), `facet-count`, `drop-hint`.
- Lists: `mod-list`, `mod-row`, `mod-tile`, `mod-main`, `mod-name`, `mod-meta`, `dot`; `profile-list`, `profile-item`, `profile-select`, `profile-meta`; `check-list`, `check-row`, `order-list`, `detected-list`.
- Settings: `section-nav`, `section-link` (`aria-current="location"`), `settings-section`, `group-title`, `card rows`, `setting-row`, `setting-row-block`, `setting-row-stack` (label line + an often-empty result, no gap), `setting-row-line`, `setting-text`, `setting-label`, `setting-control`, `setting-value`, `details`; About update result: `update-result`, `update-result-line` (the live region), `update-result-ok`, `update-result-new`, `update-result-error`, `update-result-action`.
- Badges: `badge badge-on|badge-off|badge-store`.
- Switch: `toggle` (switch + state label), `switch` (`role="switch"`, `aria-checked`, `aria-disabled`; the button is a 32x24 hit target drawing a 28x16 track with `::before`), `switch-thumb`, `toggle-label` (+ `toggle-label-on`), `toggle-hint` (the visible reason the switches are unavailable). Off: `--surface-3` track, `--border-strong` border, `--text-muted` thumb. On: `--running` track, `--bg` thumb. Unavailable: 0.45 opacity, `not-allowed` cursor.
- States: `empty-state` (+ `empty-state-compact`, `empty-icon`, `empty-title`), `loading-state`, `skeleton-list`, `skeleton-row`, `skeleton`; `session-status session-status-<kind>`, `notice notice-info|success|warning|error`, `banner banner-info|error` (update banner), `banner-icon`.
- Icons: `Icon` (`icon`, `nav-icon`, `search-icon`), names: library, profiles, settings, search, import, play, game, folder, refresh, language, filter, up, down, plus, check (up-to-date result), info (About section).

## Focus

Every interactive element gets a `:focus-visible` outline using `--focus-ring` (tabs draw it inset so the top bar does not clip it). The style feature test checks that every `:focus-visible` rule uses the token and that no rule removes an outline without painting a replacement.

## Follow-ups (need application or infrastructure work)

Not implemented in #48, suggested as separate issues:

- Game build/version and store detection beyond "has a Steam App ID".
- Mod metadata: version, author, category/type facets, thumbnails.
- Library load-order view per profile (drag to reorder) and a conflicts check between mods that ship the same files.
- Storage usage (library size, per-category bars) and an "open mods folder" action.
- Notifications history (a popover of past notices).
