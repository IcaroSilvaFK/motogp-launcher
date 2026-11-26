# Spike: ads in the launcher (issue #93)

## Decision context

On 2026-11-25 the user decided to monetize with ads in visible but non-intrusive places, plus an Apoia.se support link (separate issue #94). The developer is in Brazil. This document researches which ad sources can work in a Tauri 2 desktop app (WebView2, React UI, Rust backend) and how to integrate one without weakening the strict CSP. It is research only: nothing here is implemented.

## Method and limits of this research

- Sources are primarily the networks' own pages, fetched on 2026-11-25; some claims rest on third-party sources (review sites, forum replies, a publisher blog, law-firm summaries), and those are labelled "third-party" where they are cited and in the Sources section. Every claim carries a source URL or is marked as the project's assessment. Anything I could not confirm is marked **unverified**.
- Each candidate section states the same eight criteria, each in its own bullet: desktop allowed, approval, payout model and minimum, Brazil payouts, integration style, CSP impact, tracking and cookies, LGPD and GDPR consent. "Project's assessment" means a judgment by this spike, not a source and not legal advice.
- Not verified at all: Brazil-specific payout availability for most networks, Pix support anywhere, GDPR text (no primary GDPR source was fetched; only the Brazilian ANPD guide was found), and whether any network counts desktop-app impressions as "pageviews".
- Networks are web-first. For most of them, "is a desktop app allowed?" is simply **not addressed** in the public documentation. That is not the same as "allowed": it means the answer must come from the network's support before any integration work.

## Current code facts (grounding)

- CSP in `src-tauri/tauri.conf.json`: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'`. Images from `data:` already work; remote scripts, iframes and remote images do not.
- Network is only done from Rust. `crates/infrastructure/Cargo.toml` uses `reqwest` 0.13 (blocking, rustls). The example is `crates/infrastructure/src/catalog_http.rs` (`HttpCatalogSource`, config struct with URL, User-Agent, timeouts and a `max_bytes` cap), wrapped in `spawn_blocking` by `src-tauri`.
- Ports in Rust live in `crates/application/src/*_ports.rs` (for example `catalog_ports.rs`: `CatalogSource`, `CatalogCache`, `WebPageOpener`). Adapters live in `crates/infrastructure` and `src-tauri/src/adapters.rs`; composition is in `src-tauri/src/compose.rs`; IPC commands are in `src-tauri/src/commands.rs`.
- External links are opened from Rust: `TauriWebOpener` (`src-tauri/src/adapters.rs`) calls `tauri_plugin_opener` `open_url` after `check_page_url`, so the frontend capability stays limited to `steam://`. The Discover tab "Open page" sends only an entry id and the host shows as text. The ad click must reuse this pattern.
- Frontend ports live in `src/application/ports/` (`AppInfo.ts`, `UpdateService.ts`, `WindowControls.ts`), and small preference stores follow `discoverViewPreference.ts` / `sidebarPreference.ts` with a localStorage adapter in `src/infrastructure/`.
- UI: `Sidebar.tsx` has a `collapsed` prop and a pinned collapse toggle; `SettingsScreen.tsx` has sections `game | preferences | storage | about`; `AboutSection.tsx` is a `setting-row` card; `DiscoverScreen.tsx` has the card grid. `docs/design/ui-style.md` fixes the tokens (`--surface-1`, `--border`, `--text-muted`, `--type-*`), says the accent (`--accent`) is reserved for Play, and forbids raw font sizes outside tokens.

## Findings per candidate

### 1. EthicalAds (Read the Docs)

- **Desktop or webview allowed?** Not addressed. The publisher policy assumes web pages ("your site", "above the fold") and says nothing about apps or server-side use. Source: <https://www.ethicalads.io/publisher-policy/>. The ad server does have a server-to-server decision API (`POST /api/v1/decision/`, authentication required, you send the user's user agent), which suggests a non-JS integration is technically possible. Source: <https://ethical-ad-server.readthedocs.io/page/user-guide/api.html>. Whether the network permits it for an app is **unverified**.
- **Approval:** "Your site needs at least 50,000 pageviews per month", with occasional exceptions for fast-growing, relevant sites. Developer-focused audience. Sources: <https://www.ethicalads.io/publishers/faq/>, <https://www.ethicalads.io/publisher-guide/>. A MotoGP mod launcher is not a developer audience, so approval is unlikely.
- **Payout model:** CPM-style, estimated at about 2.50 USD per 1,000 pageviews for mostly EU and North America traffic (<https://www.ethicalads.io/publishers/faq/>). Minimum payout 50 USD, monthly (<https://www.ethicalads.io/publisher-guide/>).
- **Brazil payout:** PayPal, Open Collective, GitHub Sponsors and bank transfer via Stripe are listed (<https://www.ethicalads.io/publisher-guide/>). Availability for a Brazilian recipient is **unverified**; PayPal is the likely path.
- **Integration:** JS client snippet or the server decision API (JSON with a creative). Placement rules: one ad per page, visible on first load, at least 0.1% CTR (<https://www.ethicalads.io/publisher-policy/>). A permanently visible sidebar card and a Discover banner at once would break "one ad".
- **CSP:** none if only the Rust server API is used. The JS client would need external script and image hosts: not acceptable.
- **Tracking and cookies:** "EthicalAds serves relevant ads without using any personal data or tracking"; IPs are used at country level only and not stored (<https://www.ethicalads.io/publishers/faq/>). Server API use sets no cookies in the app (project's assessment).
- **LGPD and GDPR consent:** EthicalAds' own claim of no personal data suggests the lowest consent burden of all network candidates, but the user's IP and user agent still go to the network through Rust (the decision API takes the user agent). Whether consent is required is **unverified**; project's assessment: disclose in the privacy note and get legal advice (see Consent).
- **Ad quality:** curated developer-oriented advertisers; low malware risk. Invalid traffic (bots, VPNs, hosting IPs, repeated views) is discarded (<https://www.ethicalads.io/publisher-policy/>); users on VPNs and repeated app launches could lose impressions.

### 2. Carbon Ads / BuySellAds

- **Relationship:** Carbon's own About page says Carbon is operated by BuySellAds (<https://carbonads.net/about>); the Carbon API docs say it is built on BuySellAds infrastructure (<https://carbonads.net/docs>). BuySellAds also sells direct placements across a broad publisher catalogue (third-party comparison, <https://business.daily.dev/alternatives/carbon-ads-buysellads/>); its own publisher terms were not read, so BuySellAds-direct criteria are **unverified**. The criteria below are Carbon's.
- **Desktop or webview allowed?** The FAQ covers websites only; apps are not addressed (<https://carbonads.net/faq>). **Unverified.**
- **Approval:** "10,000 pageviews in the last 30 days", audience of "developers, designers and the people around them", hand-reviewed, reply within 7 business days (<https://carbonads.net/faq>). Not a fit for a racing-mod audience.
- **Payout model:** not stated in the pages fetched (CPM vs CPC **unverified**). Exclusive: "only our ads on your site, and never as backfill for another network" (<https://carbonads.net/faq>). If ads are ever paused (no ad requests for 14 days), see <https://carbonads.net/placement-policy>.
- **Minimums and methods:** 1 USD overall, once the method minimum is met: PayPal 20 USD (fees), check 50 USD, wire 500 USD plus a 35 USD fee, Open Collective quarterly (<https://carbonads.net/faq>). Brazil is not mentioned: **unverified**.
- **Integration:** the FAQ does not say how ads are delivered. Carbon is believed to be delivered as a JS tag (**unverified**; the API docs at <https://carbonads.net/docs> exist but were not read in detail), which this app could not load under its CSP. **Unverified** whether a server API exists for apps.
- **CSP:** a JS tag would need external `script-src` and image hosts, which the strict CSP forbids (project's assessment). A server-side API fetched from Rust would need no CSP change; availability for apps is **unverified**.
- **Tracking and cookies:** not addressed beyond a privacy-page link (<https://carbonads.net/faq>). A publisher report (third-party) says that after disabling ads with tracking, revenue was about equal to or worse than EthicalAds (<https://blog.modrinth.com/carbon-ads/>); one publisher's experience, not Carbon's position. Cookie behavior is **unverified**.
- **LGPD and GDPR consent:** not addressed in the pages fetched. **Unverified**; treat as consent-required until Carbon answers (see Consent).
- **Ad quality:** "We pick publishers and advertisers by hand" (<https://carbonads.net/faq>): low risk.

### 3. Google AdSense, AdMob, Ad Manager

#### AdSense

- **Desktop allowed:** no. The program policy lists ads that may not be "Integrated into a software application (does not apply to AdMob) of any kind, including toolbars" (<https://support.google.com/publisherpolicies/answer/48182>). A Tauri app is a software application. Rejected.
- **Approval:** n/a because the policy prohibits software apps. AdSense is also understood to require a site (**unverified**).
- **Payout model and minimum:** n/a because the policy prohibits software apps; not researched.
- **Brazil payouts:** n/a for the same reason; not researched.
- **Integration:** JS tag in a web page (project's assessment; not sourced).
- **CSP impact:** needs external scripts and iframes; not acceptable under the strict CSP (project's assessment).
- **Tracking and cookies:** Google web ad tags use cookies (**unverified** in this spike; no primary page on AdSense cookies was fetched).
- **LGPD and GDPR consent:** for personalized ads in the EEA, the UK and Switzerland, Google requires a Google-certified consent management platform (CMP) integrated with IAB's TCF, per Google's consent requirements article for the EEA, the UK and Switzerland (help article number 13554116, which Google serves under the AdSense, AdMob and Ad Manager help centers) (<https://support.google.com/adsense/answer/13554116>). LGPD handling is **unverified**.

#### AdMob

- **Desktop allowed:** no. The SDK targets Android, iOS, Unity and Flutter mobile. A Google forum answer (2023, a forum reply, not official documentation) says non-mobile platforms are not supported (<https://groups.google.com/g/google-admob-ads-sdk/c/yyFFvx8unzs>), so the current status is **unverified**; no desktop SDK exists that this stack could use. Google's "WebView API for Ads" is for Android and iOS web content (<https://developers.google.com/admob/flutter/webview>).
- **Approval:** n/a because AdMob is a mobile-SDK-only product.
- **Payout model and minimum:** n/a for the same reason; not researched.
- **Brazil payouts:** n/a for the same reason; not researched.
- **Integration:** mobile SDK only.
- **CSP impact:** n/a (no desktop integration exists); a web tag would face the same CSP problem as AdSense.
- **Tracking and cookies:** SDK uses device identifiers (**unverified**; not researched).
- **LGPD and GDPR consent:** the same Google certified-CMP and TCF requirement for the EEA, UK and Switzerland is served for AdMob (<https://support.google.com/admob/answer/13554116>). LGPD handling is **unverified**.

#### Ad Manager

- **Desktop allowed:** web content viewing frames technical requirements exist for apps (<https://support.google.com/admanager/answer/6310245>), but for mobile app frames; a desktop WebView2 is **unverified**.
- **Approval:** Ad Manager normally needs an established publisher relationship (**unverified**).
- **Payout model and minimum:** not researched; **unverified**.
- **Brazil payouts:** not researched; **unverified**.
- **Integration:** Google tags (scripts, iframes) (project's assessment).
- **CSP impact:** not compatible with the strict CSP (project's assessment).
- **Tracking and cookies:** cookies assumed for web tags (**unverified**).
- **LGPD and GDPR consent:** Google-certified CMP integrated with the TCF for personalized ads in the EEA, the UK and Switzerland (<https://support.google.com/admanager/answer/13554116>).

Verdict for the Google family: not viable.

### 4. Adsterra

- **Desktop or webview allowed?** Not addressed in anything I found. **Unverified.** Its formats are believed to be largely pop-unders, social bars and native banners aimed at websites (**unverified**: the help center index only says "Placing ads on your website", <https://help-publishers.adsterra.com/en/>).
- **Approval:** not verified (no primary page fetched).
- **Payout:** the help center lists PayPal (Hyperwallet), USDT/BTC, wire, local bank transfer, WebMoney and Paxum; Payoneer is covered by a separate article with a 100 USD minimum and euro payouts (<https://help-publishers.adsterra.com/en/articles/5385757-receiving-and-tracking-payouts>, <https://help-publishers.adsterra.com/en/articles/5720009-receiving-payouts-through-payoneer>). Other minimums conflict between sources (5, 25 or 100 USD): confirm in the dashboard. Brazil availability **unverified**.
- **Integration:** JS tag or iframe (**unverified**; assumed from how web ad networks work).
- **CSP impact:** would need `script-src` and `frame-src` additions; not compatible with the strict CSP without an isolated window (project's assessment).
- **Tracking and cookies:** assumed from how web ad tags work, not confirmed from Adsterra's privacy policy (**unverified**).
- **LGPD and GDPR consent:** consent mechanics for publishers were not found. **Unverified**; treat as consent-required (see Consent).
- **Quality:** in the project's assessment, pop-under and redirect formats (if offered; **unverified**) are intrusive and contradict "non-intrusive". Malware or scam-creative risk for the category is widely reported but I found no primary source: **unverified**.

### 5. PropellerAds

- **Desktop or webview allowed?** Not addressed. **Unverified.**
- **Payout:** minimum reported as low as 5 USD, set in the dashboard, weekly on Thursdays (<https://propellerads.com/blog/?p=14694>); the Payoneer page says wire needs 550 USD and its blog says payouts over 20 USD, so the figure conflicts (<https://www1.payoneer.com/solutions/marketplaces-networks/propellerads/>, <https://blog.payoneer.com/propellerads>). Pix not found.
- **Brazil payouts:** **unverified**.
- **Integration:** JS tag or push or pop formats (**unverified**; assumed).
- **CSP impact:** same problem as Adsterra: external script and frame hosts, not compatible with the strict CSP (project's assessment).
- **Approval:** no official page with publisher site-approval criteria was found (<https://propellerads.com/publisher/terms> describes only an application process and PropellerAds' discretion over accounts). Criteria **unverified**.
- **Tracking and cookies:** assumed to be cookie-based web ad tags; not confirmed from a primary page. PropellerAds' own privacy policy covers its website visitors and references GDPR rights (<https://propellerads.com/privacy/>), not the publisher tag behavior. **Unverified.**
- **LGPD and GDPR:** consent mechanics for publishers (required CMP, how consent signals are passed) were not found. **Unverified**; treat as consent-required (see Consent).
- **Quality:** no primary source on malware reputation was found: **unverified**. Formats are push and pop types (**unverified**); intrusive, so reject for this product.

### 6. Overwolf ads

- **Requires the Overwolf platform?** Yes in practice. "All apps must be listed and distributed through the Overwolf Appstore", "public distribution outside the Overwolf platform isn't permitted", and "Third-party monetization services aren't allowed" (<https://dev.overwolf.com/ow-native/monetization/overview>). The launcher is a standalone Tauri app, so it would have to be rebuilt as an Overwolf app. Rejected.
- **Approval:** from 2026-11-01 "Overwolf only approves apps with Overwolf monetization", apps must follow the Advertising Policy and integrate Overwolf ads, Tebex subscriptions, or both (same page).
- **Desktop allowed:** only as an Overwolf app (see above).
- **Payout model and minimum:** the page describes payments through Payoneer in USD, first week of each month, NET 60, 200 USD net minimum, amounts below it roll over (same page). The exact revenue split or share percentage was not stated in what I read: **unverified**.
- **Brazil payout:** the page lists no eligible or excluded countries, so Brazil is **unverified** (same page).
- **Integration:** Overwolf ads SDK inside an Overwolf app (same page).
- **CSP:** not described on the page. **Unverified**; in the project's assessment an Overwolf app runs in Overwolf's own runtime, not this Tauri CSP.
- **Tracking and cookies:** not described on the page. **Unverified.**
- **LGPD and GDPR consent:** the page does not describe consent or LGPD/GDPR duties; it only links an App Developer Privacy Policy and DPA, whose contents I did not read. **Unverified.**

### 7. Direct sponsors

Business model, not a policy: no source applies (all criteria are the project's assessment).

- **Desktop allowed:** yes, the creative is yours.
- **Approval:** not applicable, negotiated with the sponsor.
- **Payout model and minimum:** flat fee or per campaign, negotiated; no network minimum. At this app's size, volume is the open question.
- **Brazil payouts:** not applicable, Pix or bank transfer by agreement.
- **Integration:** your own feed (see below); rendered natively.
- **CSP impact:** none.
- **Tracking and cookies:** none from the creative. An impression beacon is fired only if the sponsor supplies its own counter URL (optional, as the creative's `impression_url`); otherwise no beacon is sent and impressions are not counted.
- **LGPD and GDPR consent:** not applicable to the creative (own feed); only the IP exposure to the feed host and the sponsor landing page after a click (see Consent).

### 8. House ads

Promote Apoia.se, the Discord or the catalog PR flow. All criteria are the project's assessment.

- **Desktop allowed:** yes.
- **Approval:** not applicable.
- **Payout model and minimum:** no revenue; useful to fill unsold slots and prove the slot works.
- **Brazil payouts:** not applicable.
- **Integration:** your own feed.
- **CSP impact:** none.
- **Tracking and cookies:** none.
- **LGPD and GDPR consent:** not applicable to the creative (own feed); same IP exposure to the feed host as the catalog.

### 9. Others

- **Self-hosted JSON ad feed:** the app fetches `ads.json` (and a creative image) from the same GitHub raw location as the catalog (<https://raw.githubusercontent.com/IcaroSilvaFK/motogp-launcher/main/catalog/catalog.json> is the existing pattern). Adding or removing a sponsor is a PR. No tracking. In the project's own assessment (not legal advice), no consent prompt is needed for the creative fetch beyond the normal IP exposure to the host, because no cookies or identifiers are set; the ANPD cookie guide (see Consent) frames consent around cookies and personal-data processing, and the IP exposure should be disclosed in the privacy note. Counting impressions for sponsors would need your own endpoint (not recommended initially: report fixed-price flat sponsorships).
- **A-ADS (crypto-paid):** publisher minimum reported as 0.001 BTC (or 1 satoshi via Faucethub), no KYC claims (third-party review sites: <https://99bitcoins.com/bitcoin-ad-networks/>, <https://coincodecap.com/a-ads-review>). Desktop policy, approval, the iframe format, cookie use (any "cookie-free" claim) and LGPD/GDPR handling are all **unverified**; CSP would need `frame-src` if iframe-based (assumed). Crypto payout in Brazil adds exchange and tax work (project's assessment).
- **Microsoft Advertising / pubCenter, Playwire, Nitropay, Unity Ads, Coinzilla:** **not researched** in this spike. Add only if the primary path fails.

## Comparison

| Candidate         | Desktop or webview allowed                       | Approval                              | Payout model and minimum                                | Brazil payout                      | Integration                               | CSP change              | Tracking or consent                          | Malware risk          |
| ----------------- | ------------------------------------------------ | ------------------------------------- | ------------------------------------------------------- | ---------------------------------- | ----------------------------------------- | ----------------------- | -------------------------------------------- | --------------------- |
| EthicalAds        | Not addressed (unverified)                       | 50k pageviews, developer audience     | About 2.50 USD CPM estimate, 50 USD minimum             | PayPal and others listed, unverified for BR | Server decision API or JS client          | None via Rust API       | No personal data or tracking (their claim)   | Low (curated)         |
| Carbon Ads        | Not addressed (unverified)                       | 10k pageviews, developers and designers | Not stated; 20 USD PayPal minimum, exclusive            | Not mentioned (unverified)         | Unverified (normally JS tag)              | Script and image hosts if JS | Not stated                                   | Low (hand-picked)     |
| AdSense           | No: prohibited in software applications          | n/a (prohibited); needs a site (unverified) | n/a (prohibited)                                  | n/a                                | JS tag (assessment)                       | Not acceptable          | Cookies unverified; certified CMP for EEA/UK/CH personalized ads | Unverified |
| AdMob             | Mobile SDKs only (forum, unverified for desktop) | n/a                                   | n/a                                                     | n/a                                | Mobile SDK                                | n/a                     | Certified CMP for EEA/UK/CH personalized ads | Unverified            |
| Adsterra          | Unverified                                       | Unverified                            | Model unverified; minimum conflicts (5 to 100 USD)      | PayPal and wire listed, unverified for BR | JS tag or iframe (unverified)      | script-src, frame-src (assessment) | Assumed cookies and consent (unverified)     | Unverified (pop formats are intrusive, project's assessment) |
| PropellerAds      | Unverified                                       | Unverified                            | Model unverified; minimum conflicts (5 to 550 USD)      | Unverified, no Pix found           | JS tag                                    | script-src, frame-src   | Assumed cookies and consent (unverified)     | High (unverified)     |
| Overwolf          | Only inside Overwolf platform                    | Only apps with Overwolf monetization (from 2026-11-01) | Split unverified; 200 USD minimum, NET 60 | Payoneer, USD; Brazil unverified   | Overwolf SDK                              | Not described (unverified) | Not described (unverified)                | Unverified |
| Ad Manager        | Unverified (app frames are mobile)               | Unverified (established publisher)    | Not researched (unverified)                             | Not researched (unverified)        | Google tags (assessment)                  | Not acceptable          | Cookies assumed (unverified); certified CMP for EEA/UK/CH personalized ads | Unverified |
| Direct sponsors   | Yes (own creative)                               | None (negotiated)                     | Flat fee, negotiated                                    | Pix or transfer, your choice       | Your JSON and image                       | None                    | None needed for the creative                 | None (you vet it)     |
| House ads         | Yes                                              | None                                  | No revenue                                              | n/a                                | Your JSON                                 | None                    | None                                         | None                  |
| Self-hosted feed  | Yes                                              | None                                  | Flat fee, negotiated                                    | Pix or transfer                    | Your JSON, fetched from Rust              | None                    | None needed                                  | None (you vet it)     |
| A-ADS             | Unverified                                       | Unverified                            | 0.001 BTC minimum (reviews)                             | Crypto only                        | Unverified (likely iframe)                | frame-src            | Cookie use unverified                        | Unverified            |

## Recommendation

**Primary: a self-hosted, curated sponsor feed (direct sponsorships and house ads), rendered natively.** Rationale:

- Every network that could serve a clean, tracking-free ad (EthicalAds, Carbon) targets developer websites with 10k to 50k monthly pageviews and has no stated policy for apps. A MotoGP mod launcher is neither a developer audience nor a website, so approval is unlikely and, if it happened, could be revoked as non-compliant. AdSense forbids software integration outright; Overwolf demands a rebuild; the pop-oriented networks conflict with "non-intrusive" and the CSP.
- A feed you control needs no approval, no cookies, no consent flow for the ad itself, fits the existing catalog pattern (`catalog_http.rs`, cache, `WebPageOpener`), and keeps the CSP untouched.
- Revenue is honest: a niche launcher's installed base is probably hundreds to a few thousand users. Assumption: revenue per month = impressions per month × CPM / 1,000, where CPM is the price per 1,000 impressions. At the EthicalAds-style estimate of 2.50 USD CPM, 1,000 users with one impression each per month is 1,000 impressions, so 1,000 × 2.50 / 1,000 = 2.50 USD per month. At that rate the 50 USD EthicalAds minimum takes about 20 months and the 200 USD Overwolf minimum about 80 months, while the 1 USD Carbon overall minimum and the 5 USD Adsterra and PropellerAds minimums (reported, conflicting) would be reached in about 1 to 2 months (project's arithmetic). Carbon's overall minimum is 1 USD, but its PayPal method minimum is 20 USD, so at about 2.50 USD per month (using the same CPM purely as an illustration; Carbon's rate is unverified) it would take roughly 8 months of accrual to reach a first PayPal payout. So the minimum is a delay at best, but the real blockers are the approval thresholds (10,000 to 50,000 monthly pageviews) and the negligible revenue either way. The realistic income is a flat sponsor fee (a sim-racing shop or community paying a monthly amount), plus Apoia.se support. Treat ads as a small supplement, not a business model.

**Fallback: apply to EthicalAds (and, in parallel, ask Carbon) with a plain-language question first**: "May a desktop app in the Discover and Settings screens use your server API, and do app opens count as pageviews?" Only integrate if they answer yes in writing and approve the app. If not, stay on the feed. The server-side API means no CSP change.

Not recommended: AdSense (prohibited), Overwolf (platform lock), Adsterra and PropellerAds (intrusive formats, JS tags, consent burden, quality risk).

## Architecture (grounded in current code)

Goal: the WebView never talks to an ad host. Rust fetches the creative, caches it, and returns a `data:` URI over IPC; the UI renders it with `img-src data:`, which the CSP already allows.

### Rust application layer (`crates/application/src`)

- `ad_ports.rs`: the ports. `AdSource` (fetch a creative or the feed: `fetch() -> Result<AdPayload, AdSourceError>`), `AdCache` (`load` / `save`, like `CatalogCache`), and `AdBeaconSink` (fire an impression beacon for a creative only when that creative carries an `impression_url`; network creatives carry one, house ads carry none, and a sponsor creative carries one only if the sponsor supplies its own counter URL, otherwise none and impressions are not counted).
- `ad_service.rs`: `AdService` picks a creative for a slot (`discover`, `sidebar`, `about`) from the payload, respects frequency caps and the user's consent and disable flags, returns a view model or `None`. It reuses `WebPageOpener` to open the click URL.
- `ad_error.rs`: `AdError`, kept separate in the same style as `catalog_error.rs`.
- Domain types in `crates/domain` (for example `AdCreative { id, slot, title, body, image, click_url, sponsor_label, impression_url, expires_at }`) with validation (HTTPS only, allowed hosts, image size cap, label required).

### Rust infrastructure (`crates/infrastructure/src`)

- `ad_http.rs`: `HttpAdSource`, a copy of the `HttpCatalogSource` pattern (blocking reqwest, User-Agent, timeouts, `max_bytes` cap). It downloads `ads.json` and the creative image (cap, for example 200 KiB, `image/png|jpeg|webp` only; refuse SVG), and encodes the image as a `data:` URI. For EthicalAds the same adapter shape would call the server decision API (send the app's User-Agent as the publisher and the user's `user_ua`) and map the response to `AdCreative`.
- `ad_cache.rs`: persists the last good payload under the data dir like `catalog_cache.rs`, with an expiry so an offline start still shows a valid creative, and nothing when it is stale.
- Impression beacon: fired from Rust (`AdBeaconSink` over reqwest) once per creative per session after the UI reports that the slot was actually rendered, and only when the creative has an `impression_url`: network creatives carry one, house ads carry none, and a sponsor creative carries one only if the sponsor supplies its own counter URL (optional). Creatives without one send nothing and their impressions are not counted. The click is never proxied, only opened.

### `src-tauri`

- `commands.rs`: `get_ad(slot)` returns the view model (title, body, `image_data_uri`, sponsor label) or `None`; `ad_clicked(slot, creative_id)` takes only the id and opens `click_url` via the existing `WebPageOpener` after `check_page_url`, same trust model as "Open page" in Discover. The frontend never receives or supplies a URL.
- `adapters.rs` / `compose.rs`: wire `AdService` with `HttpAdSource`, the cache and the existing `TauriWebOpener`.

### Frontend

- `src/application/ports/AdProvider.ts`: `AdProvider { get(slot): Promise<AdViewModel | null>; click(slot, id): Promise<void>; reportShown(slot, id): Promise<void> }`.
- `src/application/adPreference.ts` (like `discoverViewPreference.ts`): `AdPreference = 'personalized' | 'non-personalized' | 'off'` (mapped to the Settings labels "Show", "Show without personalization" and off), with `loadAdPreference` (default per Consent section; a throwing store means `off`) and `saveAdPreference`.
- `src/infrastructure/tauriAdProvider.ts` (invoke wrapper) and `localStorageAdPreferenceStore.ts`.
- `src/ui/AdSlot.tsx`: renders nothing for `null`; otherwise a bordered card with an `<img alt="">` (data URI), the title and body, a visible "Sponsored" label and a single button that calls `click`. It uses only existing tokens (`--surface-1`, `--border`, `--text-muted`, `--type-body-small`, `--radius-md`); no accent fill, because the accent is reserved for Play. Hidden when the session is starting, running, stopping or cleaning up (`selectCanStop` or the session selectors), when ads are off, when offline with no valid cache, and during skeleton loading.

### If a network only offers a JS tag

Smallest scoped change, in order of preference:

1. **Do not use it.** Pick another source.
2. **Isolated Tauri window** with its own CSP that allows only that network's script, frame and image hosts, no IPC capability, loaded from a fixed URL, and shown only in a small window or panel. Risk: the main window CSP stays strict, but the network's scripts run with cookies in a second WebView, with the malware and consent risks of the web.
3. **Never** add the host to the main window's `script-src` or `frame-src`: that would let third-party script run next to the IPC bridge.

No candidate in this spike requires option 2 or 3.

## Consent (LGPD and GDPR)

- The ANPD (Brazil) published a cookies and personal data guide on 2022-10-18 that classes advertising cookies by purpose, analyses consent and legitimate interest as legal bases, and recommends transparency and a banner (<https://www.gov.br/anpd/pt-br/centrais-de-conteudo/materiais-educativos-e-publicacoes/processo-guia-orientativo-cookies-e-protecao-de-dados-pessoais.pdf>; summary: <https://www.mattosfilho.com.br/unico/anpd-guia-cookies-protecao-dados/>). It is interpretive guidance, not binding. I did not find a primary GDPR source in this spike, so the EU analysis (ePrivacy and consent for tracking) is **unverified**; the app's users may include Europeans and should be covered by the same rule.
- With a self-hosted feed and no personal data, no cookies and no tracking, the legal exposure is small: the only data is the user's IP address reaching the feed host and any sponsor landing page after a click. The feed host (GitHub raw) already sees IPs for the catalog.
- If a third-party network is added (EthicalAds, Carbon), the user's IP and user agent are sent to that network by Rust. That network's privacy claims (for EthicalAds, no personal data and country-level targeting only, <https://www.ethicalads.io/publishers/faq/>) should be quoted in a short privacy note shown in the app.
- **Prompt:** a non-modal, dismissible bar or card (like `NoticeBar`), shown once at the first launch where ads would appear. Draft copy, pending legal review: "The launcher shows sponsored cards. They do not track you." (true only for the self-hosted feed; wording must change for any third-party network), with **Keep** and **Turn off ads** buttons. No modal, never during a session, never blocking Play.
- **Settings toggle:** a new row in Settings > Preferences: "Sponsored content" with a three-way choice (Show, Show without personalization when a network supports it, Off). For the self-hosted feed "personalization" does not exist, so the toggle degrades to on or off.
- **Default:** self-hosted feed: on, labeled, and in the project's assessment (not legal advice, pending legal review) no consent prompt is needed for the creative itself, with the toggle always available. Any third-party network: **off until the user accepts the non-modal prompt** (opt-in), to be safe under both laws.

## Placements

All placements show a "Sponsored" label, hide during a game session (`starting`, `running`, `stopping`, `cleaning up`), are never modal and never near the Play action.

1. **Discover (primary):** either a slim banner above the results toolbar or one sponsored card as the last item of the first grid row, using the same card shell (`catalog-cover` band replaced by the creative image). Not part of the catalog counts or the search results.
2. **Sidebar footer card:** a small card above the collapse toggle, hidden when the sidebar is collapsed (`collapsed` prop) and hidden at the minimum window height; no images wider than the sidebar (216px, `--sidebar-width` in `src/ui/styles.css`).
3. **Settings > About:** a quiet row in the About card ("Sponsored", one line plus link), the least visible slot, useful for a house ad or the Apoia.se prompt (issue #94).

For ad-network policy (single ad), run at most one network ad at a time, even if three slots exist; the self-hosted feed has no such limit but should stay at no more than two visible at once (Discover plus the sidebar, or the sidebar plus Settings > About), because Settings > About is a separate screen from Discover.

## Risks

- **Policy bans:** apps are not addressed by the policies of EthicalAds and Carbon; an approval for a website does not cover an app. Get written permission first.
- **Invalid traffic flags:** repeated app launches from the same IP or VPN users look like repeated views; EthicalAds discards such traffic (<https://www.ethicalads.io/publisher-policy/>). Cap impressions per creative per day.
- **Malware or scam creatives:** none with a curated feed you review; high for pop and redirect networks. Keep image-only creatives, refuse SVG, cap size, re-validate hosts and HTTPS in Rust.
- **Revenue:** likely single-digit USD per month from networks (about 2.50 USD at 1,000 impressions), under the larger payout minimums (50 USD and 200 USD), while the 1 to 5 USD minimums would be reached in about 1 to 2 months; the real blockers are the approval thresholds and the negligible revenue. Sponsor flat fees and Apoia.se are the only realistic income.
- **Privacy:** any network call leaks IP and user agent. Keep the feed on GitHub raw; document it in the Settings text and the privacy note.
- **Offline behavior:** show nothing, never an empty box or a spinner; use the cache only while it is not expired; never block startup or Discover on the ad fetch.
- **Trust and UX:** a mod launcher is used with Steam, a trusted utility. Keep the label honest, keep a one-click disable, and do not shift layout when the ad loads (reserve the slot size or insert after load without moving the Play bar).
- **Legal:** a sponsor page opened in the browser is the sponsor's responsibility, but the label "Sponsored" is needed. Possible tax and invoicing obligations for sponsorship income in Brazil are **unverified** and need an accountant.

## Proposed follow-up issues

### F1. Ad domain, ports and service (Rust)

Add the `AdCreative` domain type, `AdSource`, `AdCache` and `AdBeaconSink` ports, `AdService` and `AdError`, with fakes and BDD.

```gherkin
Feature: Choosing a sponsored creative

  Scenario: A valid creative is returned for a slot
    Given the feed has a creative for the "discover" slot
    And ads are enabled
    When the UI asks for the "discover" ad
    Then it receives the title, body, image and a "Sponsored" label
    And it does not receive the click URL

  Scenario: Ads are off
    Given the user turned ads off
    When the UI asks for any ad
    Then no creative is returned and nothing is fetched

  Scenario: A creative with a non-HTTPS click URL is rejected
    Given the feed has a creative whose click URL uses "http"
    When the feed is read
    Then that creative is dropped
```

### F2. HTTP ad source and cache (Rust infrastructure)

`HttpAdSource` fetching `ads.json` and the image (size and type caps, `data:` URI), plus a file cache with expiry.

```gherkin
Feature: Fetching and caching ads

  Scenario: Offline with a fresh cache
    Given a creative was cached 1 day ago and the expiry is 7 days
    And the network is unavailable
    When the UI asks for the ad
    Then the cached creative is returned

  Scenario: Offline with an expired cache
    Given the cached creative expired
    And the network is unavailable
    When the UI asks for the ad
    Then no creative is returned and no error is shown

  Scenario: An oversized or non-image creative is refused
    Given the feed points to a 5 MiB image
    When the creative is fetched
    Then it is rejected and the previous cache is kept
```

### F3. Ad IPC commands and click opening

`get_ad(slot)` and `ad_clicked(slot, creative_id)` in `src-tauri`, reusing `TauriWebOpener`.

```gherkin
Feature: Opening a sponsor link

  Scenario: Clicking an ad opens the system browser
    Given an ad is shown in the "discover" slot
    When the user clicks it
    Then the creative's URL opens in the default browser
    And the launcher stays in front of the game state unchanged

  Scenario: The UI cannot open an arbitrary URL
    When the UI calls "ad_clicked" with an unknown creative id
    Then nothing is opened and an error is returned
```

### F4. `AdSlot` UI and placements

`AdProvider` port, Tauri adapter, `AdSlot` component and the three placements, with tokens from `ui-style.md` and an entry in `ui-style.md`.

```gherkin
Feature: Sponsored placements

  Scenario: Discover shows a labeled sponsored card
    Given an ad is available
    When the user opens Discover
    Then a card labeled "Sponsored" is shown outside the catalog results and counts

  Scenario: The sidebar card hides when collapsed
    Given an ad is available
    When the sidebar is collapsed
    Then the sidebar ad is not shown

  Scenario: No ads while a game session is active
    Given the session is "starting", "running", "stopping" or "cleaning up"
    Then no ad is rendered in any screen

  Scenario: Play is never blocked
    Given an ad is loading
    Then the Play button is enabled and does not move
```

### F5. Consent prompt and Settings preference

`adPreference` (store, load, save), a non-modal prompt and the Settings row.

```gherkin
Feature: Ad preference

  Scenario: First launch shows a non-modal prompt
    Given the user never chose an ad preference
    When the launcher opens
    Then a dismissible bar offers "Keep" and "Turn off ads"
    And no dialog blocks the screen

  Scenario: Turning ads off removes every slot
    When the user selects "Off" in Settings > Preferences
    Then no slot renders and no ad request is made

  Scenario: The store fails
    Given the preference store throws
    Then ads are off
```

### F6. Sponsor feed and house ads (content task)

Create `catalog/ads.json` with a house ad (Apoia.se, issue #94) and the PR flow to add a sponsor; document the creative size and label rules.

Delivered in [../sponsors.md](../sponsors.md).

```gherkin
Feature: Maintaining the sponsor feed

  Scenario: A sponsor is added by pull request
    Given a contributor adds a creative to "ads.json"
    When the checks run
    Then an invalid host, size or missing label fails the check

  Scenario: An expired sponsor disappears
    Given a creative has passed its end date
    When the feed is read
    Then it is not shown
```

### F7. (Conditional) Third-party network adapter

Only if EthicalAds or Carbon confirms in writing that a desktop app is allowed: a second `AdSource` adapter (server API, opt-in consent default, single ad at a time) behind the same port.

```gherkin
Feature: Network ads behind consent

  Scenario: No network request before consent
    Given a third-party network is configured
    And the user has not accepted the prompt
    When the launcher starts
    Then no request is sent to the network

  Scenario: Only one network ad at a time
    Given the Discover and sidebar slots are visible
    Then at most one network creative is shown
```

## What the user must do manually

1. **Decide whether to look for sponsors:** list candidates (sim-racing shops, mod authors, communities) and agree on price, duration, creative size and a "Sponsored" label. Needs an invoice or Pix arrangement; check tax treatment with an accountant (**unverified**).
2. **Ask EthicalAds and Carbon Ads in writing** (their contact or apply forms) whether a desktop app, rendered natively from a server API, is allowed and whether app opens count as pageviews. Do not integrate before they answer.
3. **If approved:** create a publisher account, choose a payout method (PayPal is the most likely for Brazil; availability **unverified**), and complete any tax or identity form they request (the EthicalAds publisher policy says "we may require a government ID from you in certain circumstances to process payouts", deleted within 30 days after the next payout: <https://www.ethicalads.io/publisher-policy/>, verified 2026-11-25).
4. **Apoia.se:** create the page (issue #94) and provide its URL for the house ad and the Settings link.
5. **Do not sign up** for AdSense (software is prohibited), or Overwolf (platform lock) for this app.
6. **Privacy note:** approve the short text shown in Settings and the first-launch prompt.
7. No site or domain is required for the self-hosted feed; a custom domain is only needed if a network demands one (**unverified** for each network).

## Sources

- EthicalAds: <https://www.ethicalads.io/publishers/faq/>, <https://www.ethicalads.io/publisher-policy/>, <https://www.ethicalads.io/publisher-guide/>, <https://ethical-ad-server.readthedocs.io/page/user-guide/api.html>
- Carbon Ads / BuySellAds: <https://carbonads.net/faq>, <https://carbonads.net/placement-policy>, <https://carbonads.net/about>, <https://carbonads.net/docs>; third-party: <https://blog.modrinth.com/carbon-ads/> (publisher blog), <https://business.daily.dev/alternatives/carbon-ads-buysellads/> (comparison site)
- Google: <https://support.google.com/publisherpolicies/answer/48182>, <https://groups.google.com/g/google-admob-ads-sdk/c/yyFFvx8unzs>, <https://support.google.com/admanager/answer/6310245>, <https://developers.google.com/admob/flutter/webview>
- Google consent (CMP requirement, served under each product): <https://support.google.com/admanager/answer/13554116>, <https://support.google.com/adsense/answer/13554116>, <https://support.google.com/admob/answer/13554116>
- Third-party in the Google group: the AdMob forum reply above is a forum post, not official documentation.
- Adsterra: <https://help-publishers.adsterra.com/en/>, <https://help-publishers.adsterra.com/en/articles/5385757-receiving-and-tracking-payouts>, <https://help-publishers.adsterra.com/en/articles/5720009-receiving-payouts-through-payoneer>
- PropellerAds: <https://propellerads.com/publisher/terms>, <https://propellerads.com/privacy/>, <https://propellerads.com/blog/?p=14694>; third-party (Payoneer pages, not PropellerAds' own): <https://www1.payoneer.com/solutions/marketplaces-networks/propellerads/>, <https://blog.payoneer.com/propellerads>
- Overwolf: <https://dev.overwolf.com/ow-native/monetization/overview>
- A-ADS reviews (third-party): <https://99bitcoins.com/bitcoin-ad-networks/>, <https://coincodecap.com/a-ads-review>
- LGPD: <https://www.gov.br/anpd/pt-br/centrais-de-conteudo/materiais-educativos-e-publicacoes/processo-guia-orientativo-cookies-e-protecao-de-dados-pessoais.pdf>, <https://www.mattosfilho.com.br/unico/anpd-guia-cookies-protecao-dados/> (third-party law-firm summary)
