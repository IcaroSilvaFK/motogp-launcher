# Sponsor feed

The launcher reads a self-hosted sponsor feed, `catalog/ads.json`, from the same GitHub raw location as `catalog/catalog.json`. The format is described by `catalog/ads.schema.json`. Background and design: [research/ads.md](research/ads.md).

## Creative rules

| Field | Rule |
|-------|------|
| `id` | Unique lowercase slug (`^[a-z0-9]+(-[a-z0-9]+)*$`), at most 64 characters |
| `slot` | `discover`, `sidebar` or `about` |
| `title` | 1 to 60 characters |
| `body` | 1 to 140 characters |
| `sponsor_label` | Required, not blank, at most 40 characters (for example `Sponsored` or `House ad`). Always shown next to the ad |
| `image` | Repo-relative path under `catalog/ads/` (no leading `/`, no `..`, no `\`) or an `https` URL. `.png`, `.jpg`, `.jpeg` or `.webp` only (no SVG). Files in the repository must be at most 200 KiB |
| `click_url` | `https` only, public host name: no user info, no port, no `localhost`, no IP address |
| `impression_url` | Optional, same rules as `click_url` |
| `starts_at`, `expires_at` | Optional, `YYYY-MM-DD`; `starts_at` must not be after `expires_at`. An expired creative is only a warning in the check (the launcher's ad service hides it at runtime, implemented in #101/#102); remove it by PR as housekeeping |

The feed accepts only the fields above and `version` must be `1`.

## Add a sponsor by PR

1. Fork the repository and add the creative image under `catalog/ads/` (at most 200 KiB, no third-party brand assets without permission).
2. Add an object to `creatives` in `catalog/ads.json` following the rules above. Always set a visible `sponsor_label`.
3. Run `cargo test -p motogp-infrastructure --test ads_file`. It reports every problem with the creative id and field.
4. Open a PR. Once it reaches `main`, launchers see the change after their cache expires.
5. When the campaign ends, remove the creative (and its image) by PR as housekeeping. Once `expires_at` is in the past the launcher's ad service hides it at runtime (implemented in #101/#102) and the check only prints a warning, it does not fail.

The house ad `house-support` points at the support page (`SUPPORT_PAGE_URL` in `crates/domain/src/support_page.rs`). Keep both in sync.
