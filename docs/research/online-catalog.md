# Spike: online mod catalog from Overtake (issue #37)

Date: 2026-11-19. Status: research only, no production code.

## Method and limits of this research

Rules followed: robots.txt respected, a handful of manual requests, no crawling, no bypass of logins, CAPTCHAs or Cloudflare.

**Primary finding: overtake.gg is behind a Cloudflare managed challenge.** Every request made with a plain HTTP client (identifying User-Agent `motogp-launcher-research/0.1`) returned `HTTP 403` with the "Just a moment..." interstitial (`cType: 'managed'`, requires JavaScript and a Turnstile token). This included:

| URL | Result |
|-----|--------|
| https://overtake.gg/robots.txt | 403, Cloudflare challenge |
| https://overtake.gg/help/terms/ | 403, Cloudflare challenge |
| https://overtake.gg/downloads/ | 403, Cloudflare challenge |
| https://overtake.gg/downloads/rebalanced-ai.69345/ | 403, Cloudflare challenge (also via the WebFetch tool) |
| https://overtake.gg/downloads/index.rss | 403, Cloudflare challenge |
| https://overtake.gg/api/ | 403, Cloudflare challenge |

Consequences:
- Terms, robots.txt, resource page fields and file hosting could **not be verified first-hand**. Anything below marked "unverified" is from general XenForo knowledge and must be confirmed by a human in a normal browser before any implementation.
- A web search for the site's terms returned nothing from overtake.gg itself (only unrelated third-party terms and a Ghostery tracker page).
- Most importantly, the challenge is itself a signal: the site owner deliberately blocks non-browser clients. A Rust `reqwest` client from the launcher would hit the same wall, and solving it (headless browser, cookie replay, challenge solvers) is exactly the bypass this spike rules out.

## Findings per question

### 1. Permission

- Verified: automated access without a browser is actively blocked by Cloudflare (above). Treat this as "automated access not sanctioned" until the owners say otherwise.
- Unverified: the written terms (https://overtake.gg/help/terms/ and the site rules) could not be read. Forums of this type almost always forbid automated scraping and restrict redistribution of member content; assume so.
- Unverified: XenForo ships a REST API (`/api/...`) that requires an API key issued by the admin, with per-key scopes and permissions. With the Resource Manager / Downloads add-on, key-scoped endpoints could in principle expose resources, but only if the owners create a key for us. The `/api/` URL answered with the Cloudflare challenge, so even key-authenticated calls from a server-side client may need an owner-side Cloudflare allow rule (WAF skip for the key or IP/header).
- Unverified: RSS. XenForo exposes `index.rss` for forums and, depending on version and add-on, for resource categories. The tested RSS URL was challenged as well, so RSS is not usable from a non-browser client either.
- Conclusion: the only sanctioned channel we can count on is **asking the site owners** (staff or admin via the site's contact/PM channel) for either an API key plus a Cloudflare exception, or written permission for a defined integration (read-only listing, links back to the mod page, attribution).

### 2. Data per mod

Unverified (page not retrievable). Typical XenForo resource pages expose: title, tag line, author (member), version string, category, first release and last update dates, rating (average and count), download count, view count, description (BBCode), thumbnail/cover image, screenshots, and a Download button. "Game" (MotoGP 24 vs others) is usually the category tree, not a separate field. The launcher's catalog model should therefore only require: `id`, `title`, `author`, `version`, `category`, `updated_at`, `thumbnail_url`, `page_url`, and treat rating and download count as optional.

### 3. Where files live

Unverified. In XenForo Resource Manager a resource is either an attached file hosted by the site (download often requires being logged in, and sometimes accepting a licence or being a member of a group) or an external URL (Google Drive, Mega, Patreon, Discord). MotoGP mod authors commonly use both. Implications:
- Site-hosted attachments behind login cannot be downloaded by the launcher without handling the user's credentials, which we must not do.
- External hosts (Drive, Mega) have their own interstitials, quotas and CAPTCHAs; not reliably automatable and out of scope.
- Therefore the safe, honest contract is **open the mod page in the user's default browser**; the user downloads manually and imports with the existing flow (drag and drop / file picker).

Sample of 5 popular mods listed in the task could not be inspected (all blocked), so no per-mod hosting statistics are available.

### 4. Architecture (grounded in current code)

- CSP in `src-tauri/tauri.conf.json`: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'`. The webview cannot fetch remote data or load remote images. All network access must live in Rust, and thumbnails would have to be fetched by Rust and passed as `data:` URLs (or cached on disk and served through a Tauri command), without loosening the CSP.
- Ports live in `crates/application/src/*_ports.rs` (e.g. `ModRepository`, `LibraryFiles`, `ArchiveReader`, `ContentHasher`). A catalog would add:
  - `CatalogSource` (application port): `fn list(&self) -> Result<Vec<CatalogEntry>, CatalogError>`; implementations: `EmbeddedCatalog` (JSON bundled or fetched from this repo), later possibly `OvertakeApiCatalog`.
  - `CatalogCache` (port): last-good snapshot on disk with `fetched_at`, so the UI works offline and does not hit the source on every start (TTL e.g. 6 h, conditional requests with ETag).
- Import pipeline (#3): `ModService::import(&Path)` takes a local file, `ArchiveReader::extract_flat` (infrastructure `zip_archive.rs`) enforces limits (`max_entry_bytes` 8 GiB, `max_total_bytes` 16 GiB, `max_entries` 10 000, unsafe paths ignored) and `ContentHasher` computes SHA-256. An in-app download, if ever permitted, would stream to a temp file with a size cap and feed `import(path)` unchanged; no new extraction path is needed. Curated entries can carry an optional expected `sha256` to verify after manual download.
- Rate limits: single-flight refresh, TTL, exponential backoff, honour `Retry-After`, identify with a descriptive User-Agent.

## Options compared

| Option | Permission | Works with Cloudflare | Effort | Risk | Verdict |
|--------|-----------|-----------------------|--------|------|---------|
| A. Official XenForo API with key | Needs owner approval | Needs owner-side WAF exception | Medium | Dependent on owners; keys can be revoked | Best long term, only after permission |
| B. RSS feeds | Unclear, likely tolerated only for readers | Blocked (403 challenge) | Low | Same wall as scraping | Rejected for now |
| C. Curated JSON of links in this repo, open in browser | None needed (we only link) | Not applicable (user's browser) | Low | Maintenance burden; links go stale | **Recommended now** |
| D. Scraping HTML / headless browser | Almost certainly forbidden; circumvents Cloudflare | Requires bypassing the challenge | High | ToS violation, legal exposure, IP bans, fragile, breaks CSP model | **Rejected** |

Why scraping is rejected: the site actively blocks it with a managed challenge, the issue forbids bypassing Cloudflare/CAPTCHAs, terms are presumed to forbid automation, and redistributing member-authored files without consent is a licensing problem independent of the technical one.

## Recommendation

1. Ship **Option C** first: a curated, reviewable `catalog/catalog.json` (title, author, version, category, page URL, optional thumbnail, optional sha256), shown in a "Discover" tab; the action is "Open page" in the system browser, then the normal import.
2. In parallel, **contact the Overtake staff** to request (a) permission to link and show titles/thumbnails and (b) an API key with read-only resource scope plus a Cloudflare exception. If granted, add Option A behind the same `CatalogSource` port, so the UI does not change.
3. Never implement B or D.

## Risks

- Outdated links and versions in the curated list (mitigate: CI link-format validation, `updated_at`, contribution template).
- Thumbnails: hotlinking or redistributing images may need consent; ship without thumbnails or with user-uploaded ones until permission exists.
- Opening external URLs from the webview needs an allow-listed Tauri command (`https` only, host allow-list), not a CSP change.
- Owners may refuse or not answer: Option C remains valid on its own.
- All "unverified" items must be checked by a person in a normal browser before the follow-up issues are finalised.

## Proposed follow-up issues

### F1. Curated catalog JSON and Rust source adapter

```gherkin
Feature: Curated mod catalog
  Scenario: Load the bundled catalog
    Given the app ships a catalog.json with 3 valid entries
    When the catalog is requested
    Then 3 entries are returned with title, author, category and page URL

  Scenario: Reject malformed entries
    Given the catalog contains an entry with a non-https page URL
    When the catalog is loaded
    Then that entry is skipped and a warning is recorded
    And the valid entries are still returned
```

### F2. Catalog cache for offline use

```gherkin
Feature: Catalog cache
  Scenario: Use cache while fresh
    Given the catalog was fetched 1 hour ago and the TTL is 6 hours
    When the catalog is requested
    Then the cached snapshot is returned without a network call

  Scenario: Work offline
    Given the cache is stale and the network is unavailable
    When the catalog is requested
    Then the stale snapshot is returned with a "stale" flag
```

### F3. Discover tab with "Open page in browser"

```gherkin
Feature: Discover tab
  Scenario: Browse and filter
    Given the catalog has mods in categories "Helmets" and "AI"
    When I filter by "AI"
    Then only AI mods are listed

  Scenario: Open a mod page
    When I click "Open page" on a mod
    Then its https URL opens in the default browser
    And the app webview does not navigate away

  Scenario: Block unsafe URLs
    Given an entry with a "file://" URL
    When I click "Open page"
    Then nothing opens and an error is shown
```

### F4. Optional checksum verification on import

```gherkin
Feature: Verify imported catalog mod
  Scenario: Matching checksum
    Given a catalog entry with sha256 X
    When I import a file whose SHA-256 is X
    Then the mod is linked to that catalog entry

  Scenario: Different checksum
    When I import a file with another SHA-256
    Then the import proceeds as a normal local mod and a notice says it does not match the catalog
```

### F5. Ask Overtake staff for API access (non-code task)

```gherkin
Feature: Owner permission
  Scenario: Decision recorded
    Given a message was sent to the Overtake staff describing the integration
    When they reply
    Then the answer and terms are saved in docs/research/online-catalog.md
    And an API-backed CatalogSource issue is opened only if access was granted
```

### F6. (Conditional on F5) Official API catalog source

```gherkin
Feature: API-backed catalog
  Scenario: Fetch with key
    Given a granted read-only API key stored outside the repo
    When the catalog refreshes
    Then entries are mapped to CatalogEntry through the CatalogSource port

  Scenario: Respect rate limits
    Given the API answers 429 with Retry-After 60
    When a refresh is attempted
    Then no new request is sent for 60 seconds
```
