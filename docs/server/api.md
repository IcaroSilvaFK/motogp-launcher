# Community server API

Base URL: `PUBLIC_BASE_URL` (HTTPS in production, `http://127.0.0.1:8787` in development). All bodies are JSON.

## Error format

Every error, on every endpoint (including unknown routes), uses the same shape and a matching HTTP status:

```json
{ "error": { "code": "not_found", "message": "The resource was not found." } }
```

| Code | Status | Meaning |
|------|--------|---------|
| `bad_request` | 400 | Malformed or invalid input |
| `invalid_redirect` | 400 | Sign-in redirect is not an allowed loopback URL |
| `invalid_code` | 400 | One-time code unknown, used, expired or wrong verifier |
| `unauthorized` | 401 | Missing, unknown, expired or revoked session |
| `forbidden` | 403 | Not an admin, or not the owner of the upload |
| `banned` | 403 | The account is banned from community writes (comments, reports, stars, uploads); reads still work |
| `not_found` | 404 | Unknown route or resource |
| `mod_id_taken` | 409 | The mod id belongs to another creator |
| `version_exists` | 409 | A non-rejected version with this number already exists |
| `offset_mismatch` | 409 | The chunk offset is not the bytes received; the error carries `received` |
| `upload_incomplete` | 409 | Completing before every byte was received |
| `file_too_large` | 413 | Declared size over the cap (refused before storing), or chunk/screenshot over its limit |
| `quota_exceeded` | 413 | The per-user storage quota would be exceeded |
| `unsupported_file` | 400 | Extension is not `.zip`/`.7z`/`.pak`, or the screenshot is not PNG/JPEG/WebP |
| `checksum_mismatch` | 400 | The server SHA-256 differs from the `sha256` the client sent |
| `rate_limited` | 429 | Too many requests; see the `Retry-After` header (seconds) |
| `discord_unavailable` | 502 | Discord could not be reached or answered with an error |
| `internal` | 500 | Unexpected failure; details are logged, never returned |

Messages are English sentences meant for logs, not for display. Clients should switch on `code`.

## Rate limits

In-memory token buckets (burst = the per-minute limit, refilled continuously), keyed by client IP or user id:

| Group | Key | Limit |
|-------|-----|-------|
| All routes | client IP | 60 / minute |
| `/auth/*` | client IP | 10 / minute |
| Authenticated routes | user id | 120 / minute |
| Comment writes (`POST`, `PATCH`, `DELETE` under `/mods/{mod_id}/comments`) | user id | 5 / minute (own bucket, on top of the 120 / minute user limit) |
| `POST /mods/{id}/downloads` | client IP | 20 / minute (on top of the global limit) |

When a bucket is empty the server answers `429 rate_limited` with `Retry-After: <seconds>`. Idle buckets are evicted.
The client IP is the connection address; with `TRUST_PROXY_HEADERS=true` it is the right-most `X-Forwarded-For`
value (set by Caddy).

## CORS

Closed: the server never sends `Access-Control-Allow-Origin`. The launcher talks to the server from Rust, not from a
browser origin.

## Endpoints

### `GET /health`

`200 {"status":"ok"}` when the database answers. A database failure answers `500 internal`.

### Authentication endpoints

Sign-in uses Discord OAuth2 (scope `identify`) with PKCE on both legs: the server talks to Discord with its own
PKCE pair, and the desktop app proves possession of its own verifier when it trades the one-time code. The `/auth/*`
routes share the stricter 10 / minute per-IP limit.

#### Desktop sign-in flow

```
App                         Browser                 Community server              Discord
 | bind 127.0.0.1:0, make state + PKCE pair (verifier, challenge = B64URL(SHA256(verifier)))
 |--- open ----------------->| GET /auth/discord/start?redirect=...&state=...&code_challenge=...
 |                           |------------------------>| validate redirect, store flow (10 min)
 |                           |<-- 302 discord.com/oauth2/authorize?...&code_challenge_method=S256 --|
 |                           |-------------------------------------------------------> user consents
 |                           |<-- 302 {PUBLIC_BASE_URL}/auth/discord/callback?code&state ------------|
 |                           |------------------------>| exchange code at Discord, read /users/@me,
 |                           |                         | upsert user, create one-time code (60 s)
 |<-- 302 http://127.0.0.1:<port>/callback?code=<otc>&state=<app state> --|
 |--- POST /auth/token {code, code_verifier} -------->|
 |<-- 200 {token, expires_at, user} ------------------|
 |--- GET /me  (Authorization: Bearer <token>) ------>|
```

The app must check that the returned `state` equals the one it generated.

#### `GET /auth/discord/start`

Query: `redirect`, `state`, `code_challenge`.

- `redirect` must be exactly `http://127.0.0.1:<1024-65535>/callback` (no userinfo, other host, other path, query or
  fragment), else `400 invalid_redirect`.
- `state`: 1 to 128 characters of `A-Z a-z 0-9 - . _ ~`. `code_challenge`: 43 characters of the same set (a
  base64url SHA-256 digest). Missing or malformed values answer `400 bad_request`.
- `302` to Discord's authorize URL. The flow expires after 10 minutes. Expired flows and codes are purged on each call.

#### `GET /auth/discord/callback`

Query (sent by Discord): `code` and `state`, or `error` and `state`.

- Unknown, expired or already used `state`, or a missing `code` / `state`: `400 bad_request`.
- `error=access_denied` (user pressed Cancel): `302` to `http://127.0.0.1:<port>/callback?error=access_denied&state=<app state>`.
  Any other Discord `error` redirects with `error=sign_in_failed`.
- Success: `302` to `http://127.0.0.1:<port>/callback?code=<one-time code>&state=<app state>`.
- Discord unreachable or answering with an error: `502 discord_unavailable`.

The flow is consumed on the first attempt, so a callback URL cannot be replayed.

The same callback finishes the admin web sign-in (`GET /admin/login`, see [Admin web](#admin-web)): for such a flow it
needs the `admin_login` cookie and answers an HTML page with the `admin_session` cookie instead of a loopback redirect.

#### `POST /auth/token`

Request: `{"code":"<one-time code>","code_verifier":"<app PKCE verifier>"}`.

Response `200`:

```json
{
  "token": "<opaque, 43+ characters>",
  "expires_at": "2026-12-26T10:00:00Z",
  "user": { "id": "...", "discord_id": "...", "username": "...", "display_name": "...", "avatar_url": null }
}
```

Errors: `400 invalid_code` (unknown, already used, expired code or wrong verifier; the code is consumed on any
attempt), `400 bad_request` (malformed JSON or missing fields).

The token is 32 random bytes (base64url). Only its SHA-256 hash is stored. Sessions last 30 days and can be revoked.

#### `GET /me`

Header `Authorization: Bearer <token>`. Response `200`:

```json
{
  "id": "<internal user id>",
  "discord_id": "123456789012345678",
  "username": "rider",
  "display_name": "Rider One",
  "avatar_url": "https://cdn.discordapp.com/avatars/<discord_id>/<hash>.png?size=64"
}
```

`display_name` is the Discord global name, else the username. `avatar_url` is `null` when the user has no avatar.
Errors: `401 unauthorized` (missing header, unknown, expired or revoked token).

#### `POST /auth/logout`

Header `Authorization: Bearer <token>`. Response `204` with no body; the session is revoked. Calling it again with
the same token answers `401 unauthorized`.

#### The `AuthenticatedUser` extractor

Any later route (comments, stars...) requires a session by taking `AuthenticatedUser` (`community_api::auth`) as a
handler argument:

```rust
async fn my_route(auth: AuthenticatedUser) -> Result<Json<Value>, ApiError> {
    // auth.user: the community_domain::User, auth.token: the presented SessionToken
    Ok(Json(json!({ "hello": auth.user.display_name() })))
}
```

It reads `Authorization: Bearer <token>`, resolves the session through `AuthService::authenticate`, applies the
per-user rate limit (`429 rate_limited` with `Retry-After`) and rejects with `401 unauthorized` when the token is
missing, unknown, expired or revoked. Register the route in `routes::extra_routes()`; no other wiring is needed.

### Stars

Mod ids are catalog slugs (`^[a-z0-9]+(-[a-z0-9]+)*$`, 1 to 100 characters); the server does not know the
catalog, so any valid slug is accepted. One star per (user, mod): table `mod_stars`, migration
`server/migrations/0003_stars.sql` (`user_id` references `users(id)` with `ON DELETE CASCADE`).

#### `PUT /mods/{id}/star` and `DELETE /mods/{id}/star`

Require `Authorization: Bearer <token>`. Both are idempotent: starring twice keeps one star, and unstarring a
mod that is not starred succeeds.

```json
{ "id": "ducati-2025", "stars": 11, "starredByMe": true }
```

`DELETE` answers the same shape with `"starredByMe": false`. Errors: `400 bad_request` (invalid id),
`401 unauthorized`, `429 rate_limited`.

#### `GET /mods/stats`

Optional authentication. Lists only mods with at least one star, ordered by id.

```json
{ "mods": [{ "id": "ducati-2025", "stars": 11, "starredByMe": false }] }
```

`starredByMe` is present only when a valid bearer token is sent (that call also pays the per-user rate limit).
A missing, unknown, expired or revoked token is answered anonymously (no `starredByMe`, never `401`), so counts
keep working when a session goes stale.

### Mod comments

Comments are plain text attached to a mod by its catalog slug. The server does not know the catalog, so any valid
`mod_id` is accepted. Storage is the `comments` table, created by migration `0002_comments.sql`
(`id`, `mod_id`, `user_id` -> `users` with `ON DELETE CASCADE`, `body`, `created_at` and `edited_at` in epoch
milliseconds; index on `(mod_id, created_at, id)`).

**Limits**

| What | Rule |
|------|------|
| `mod_id` | 1 to 100 characters of `a-z 0-9 -`; anything else is `400 bad_request` (on every method) |
| Body | 1 to 2000 characters (counted in `char`s) after trimming; `\r\n` is normalized to `\n`; control characters other than `\n` and `\t` are rejected |
| Page size | `limit` 1 to 50, default 20 |
| Writes | 5 per minute per user (`POST`, `PATCH` and `DELETE` share one bucket); the 6th answers `429 rate_limited` with `Retry-After` |
| Request body | JSON, at most 16 KiB; larger or malformed bodies answer `400 bad_request` |

Bodies are stored and returned verbatim: HTML is never interpreted or escaped by the server. Clients must render
the body as text (the launcher uses its safe Markdown renderer).

**The `Comment` shape**

```json
{
  "id": "q3Zk...",
  "body": "Great livery",
  "created_at": "2026-11-26T10:15:30.123Z",
  "edited_at": null,
  "author": { "id": "...", "display_name": "Ada", "avatar_url": "https://cdn.discordapp.com/avatars/.../....png?size=64" },
  "own": false
}
```

`created_at` and `edited_at` are RFC 3339 UTC with millisecond precision (`edited_at` is `null` until the first edit).
`author.avatar_url` is `null` when the user has no avatar. The Discord id and username are never exposed. `own` is
`true` only when the caller presented a valid token and wrote the comment.

| Method and path | Auth | Success | Errors |
|-----------------|------|---------|--------|
| `GET /mods/{mod_id}/comments?cursor=&limit=` | Optional bearer: it only sets `own`; a missing, bad or expired token is ignored and never answers 401 | `200 {"comments":[Comment...],"next_cursor":string\|null}`, oldest first (newest last) | `400` bad mod id, cursor or limit |
| `POST /mods/{mod_id}/comments` with `{"body"}` | Required | `201` with the `Comment` | `400`, `401`, `429` |
| `PATCH /mods/{mod_id}/comments/{comment_id}` with `{"body"}` | Required, author only | `200` with the `Comment` (`edited_at` set) | `400`, `401`, `404`, `429` |
| `DELETE /mods/{mod_id}/comments/{comment_id}` | Required, author only | `204` with no body (hard delete) | `400` (bad mod id), `401`, `404`, `429` |

**Pagination.** Pages are keyset-based on `(created_at, id)` ascending, so posting a comment never shifts a page.
`next_cursor` is opaque and present only when more comments follow; pass it back unchanged as `cursor`. A malformed
cursor is `400 bad_request`. Comments created in the same millisecond are ordered by id.

**Ownership.** A comment that does not exist, belongs to another mod, or belongs to another user answers the same
`404 not_found` for `PATCH` and `DELETE`, so callers cannot probe which ids exist. Writes check the per-user write
limit before the body is validated, so rejected writes also count.
### Moderation

Moderators are the Discord ids in `ADMIN_DISCORD_IDS`. They work in the [admin web](#admin-web) (a server-rendered
page, no JavaScript); the JSON API only has the report endpoint for users plus the mod review routes below.

#### `POST /mods/{mod_id}/comments/{comment_id}/reports`

Bearer required. Body `{"reason": "spam"|"abuse"|"off_topic"|"other", "details"?: string}` (JSON, at most 16 KiB;
`details` is trimmed and at most 500 characters). It shares the comment write bucket (5 per minute per user).

- `204` on success. It is idempotent per (user, comment): a second report answers `204` and is not duplicated.
- `400 bad_request` for a bad reason or details, or when you report your own comment.
- `404 not_found` when the comment is unknown, belongs to another mod or is hidden.
- `401`, `403 banned`, `429` as usual.

#### Hidden comments

An admin can hide a comment with a reason. A hidden comment disappears from `GET /mods/{mod_id}/comments` and
answers `404 not_found` to `PATCH`, `DELETE` and reports. Hiding resolves the comment's open reports; restoring puts
it back. Comments are never deleted by moderation.

#### Bans and `banned`

An admin can ban a user (with a reason). A banned user gets **`403 banned`** on every write: comment `POST`, `PATCH`
and `DELETE`, reports, star `PUT` and `DELETE`, and every upload write (create, chunk, screenshots,
complete, abort; `GET /mods/uploads/{id}` stays readable). The check runs after the session check and before the route's own limiter, so a banned user always
sees `banned`, never `429`. Reads, sign-in and `GET /me` keep working. While the ban lasts, the user's comments are
hidden from `GET`; unbanning shows them again (except those an admin hid one by one). Admins cannot ban themselves
or another admin (`400`). The launcher maps `banned` to a "your account is suspended" message.

#### Audit

Every hide, restore, ban, unban, report dismissal, mod approval and mod rejection writes one row in
`moderation_actions` (admin, action, target, reason, time; migration `0006_moderation.sql`). The target is a comment
id, a user id, or `mod@version`. The admin page shows the last 50 entries. Rows keep their text when the admin's user
row is later deleted (the admin shows as "(deleted user)").

#### Admin web

Server-rendered HTML under `/admin`, with a minimal inline stylesheet (`api/src/admin_css.rs`) and no JavaScript.

**Sign-in.** `GET /admin/login` starts a Discord sign-in (the same Discord application and the same redirect URI,
`/auth/discord/callback`) and answers `302` to Discord. It sets `admin_login=<state>` (HttpOnly, Secure,
SameSite=Lax, Path=/, Max-Age 600). The callback requires that cookie to equal the OAuth `state` (login CSRF), else
`400`. A user whose Discord id is not in `ADMIN_DISCORD_IDS` gets a `403` page and no session. An admin gets a
12-hour session (`sessions` table), `Set-Cookie: admin_session=<token>; HttpOnly; Secure; SameSite=Strict;
Path=/admin; Max-Age=43200`, and a `200` page that forwards to `/admin` with
`<meta http-equiv="refresh" content="0;url=/admin">`. It is not a `302` on purpose: a `SameSite=Strict` cookie is
not sent on a redirect chain that started on another site (Discord), so a redirect would land on the login page.
The page makes the browser start a new, same-site navigation that carries the cookie. App sign-ins
(`/auth/discord/start`) are unchanged.

`Secure` cookies need HTTPS, except that browsers also accept them on `http://localhost`. Behind Caddy this is the
normal setup.

**Admin is re-checked on every request.** The cookie only proves the session; the Discord id must still be in
`ADMIN_DISCORD_IDS` each time, so removing an id and restarting locks that admin out at once (`403`). Admin HTML
routes take only the cookie, never a bearer token. They share the global IP limiter, and each request also counts
against the per-user limit.

**CSRF.** Every `POST` form carries a hidden `csrf` field, `hash("csrf:" + session token)`, compared in constant
time. A missing or wrong token answers `403` and changes nothing. A successful `POST` answers `303` to `/admin`;
errors render a small HTML page with the right status (`400` bad input or "cannot ban an admin", `404`, `403`, ...).

**Headers on every `/admin` response:** `Cache-Control: no-store`, `X-Frame-Options: DENY`,
`Referrer-Policy: no-referrer`, `X-Content-Type-Options: nosniff` and
`Content-Security-Policy: default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'`.
All user content (comments, names, reasons, mod metadata) is HTML-escaped.

| Route | What it does |
|-------|--------------|
| `GET /admin` | Dashboard: open reports, versions waiting for review, hidden comments, banned users (plus a ban-by-Discord-id form) and the last 50 audit entries. Without a valid cookie it shows the sign-in page (`200`); a non-admin session gets `403`. |
| `GET /admin/login` | Starts the sign-in (see above). |
| `POST /admin/comments/{id}/hide` (`reason`) | Hides a comment and resolves its open reports. |
| `POST /admin/comments/{id}/restore` | Restores a hidden comment. |
| `POST /admin/reports/{id}/dismiss` | Resolves one report as dismissed. |
| `POST /admin/users/{user_id}/ban` (`reason`) | Bans a user by internal id. |
| `POST /admin/users/ban` (`discord_id`, `reason`) | Bans by Discord id (the user must have signed in once). |
| `POST /admin/users/{user_id}/unban` | Lifts a ban. |
| `POST /admin/review/{mod}/{version}/approve` and `.../reject` (`reason`) | Approve or reject a pending version, same use cases as the JSON routes. |
| `POST /admin/logout` | Revokes the session and clears the cookie. |
| `GET /admin/mods/{mod}/versions/{version}/file` | The pending or approved file as `Content-Disposition: attachment` (with `Range`). |
| `GET /admin/mods/{mod}/versions/{version}/screenshots/{n}` | A screenshot with its sniffed image type and `nosniff`. |

Reasons are trimmed, 1 to 500 characters, and required for hide, ban and reject. The form routes for review live
under `/admin/review/...` so they never collide with the bearer-JSON `/admin/mods/...` routes, which keep working
unchanged (for curl).

### Download counts

Anonymous routes (no session). A mod id is a catalog slug: 1 to 64 characters of `a-z 0-9 -`. Days are UTC calendar days.
Every failure uses the standard error format (`bad_request`, `rate_limited`).

#### `POST /mods/{id}/downloads`

Body: `{"install_id": "<22-128 chars of A-Z a-z 0-9 - _>"}`. Answers `204` with no body, whether the download was
counted now or already counted today. `400 bad_request` for a malformed mod id, install id or JSON body.
`429 rate_limited` with `Retry-After` after 20 reports per minute from one IP.

**Dedup rule:** one count per (mod, install id, UTC day). Reporting the same mod twice on one day counts once; the
next day counts again.

**Privacy:** only `SHA-256(install_id)` (lowercase hex) is stored in `mod_downloads`, never the raw install id and
never the IP address. The install id is not logged. The table is `(mod_id, install_hash, day)` where `day` is the
number of days since 1970-01-01 UTC (migration `0004_downloads.sql`).

#### `GET /mods/downloads`

`200 {"mods":[{"id":"x","total":101,"last_30_days":7}]}`: only mods with at least one download, sorted by id.
`last_30_days` covers the same 30 buckets as the daily route (today and the 29 days before).

#### `GET /mods/{id}/downloads/daily`

`200 {"id":"x","days":[{"date":"2026-10-28","count":0}, ...]}`: exactly 30 buckets, oldest first, the last one is
today (UTC), zero for days without downloads. An unknown mod answers 30 zero buckets. `400 bad_request` for a
malformed mod id.

## Mod hosting

Creators upload mod files to the server; an admin approves them; the launcher lists approved mods in its catalog
and downloads the files from `/files`. Everything below uses camelCase JSON unless it says "raw bytes".

### Upload flow

```
Creator app                                   Community server
 | POST /mods/uploads {modId, title, ..., size, sha256?, ownershipConfirmed}
 |<-- 201 {uploadId, chunkSize: 8388608, received: 0, expiresAt}   (refused here: too large, quota, daily limit)
 | PUT /mods/uploads/{id}/chunks?offset=0       raw bytes (<= chunkSize)
 |<-- 200 {received}
 |   ... connection drops ...
 | GET /mods/uploads/{id}
 |<-- 200 {received, size, state: "uploading"}  -> continue from `received`
 | PUT /mods/uploads/{id}/chunks?offset=<received> ...
 | POST /mods/uploads/{id}/screenshots          raw PNG/JPEG/WebP, up to 8
 | POST /mods/uploads/{id}/complete
 |<-- 201 {modId, version, status: "pending", sha256, ...}
```

Rules: the declared `size` is checked against the cap (`MOD_MAX_UPLOAD_BYTES`) **before** anything is stored, and the
bytes received are checked again chunk by chunk. File names are limited to letters, digits, space, `.`, `_`, `-`,
`(` and `)`, with a `.zip`, `.7z` or `.pak` extension (case-insensitive). The `modId` follows the catalog id rule
(`^[a-z0-9]+(-[a-z0-9]+)*$`, at most 64; `mine`, `uploads` and `stats` are reserved). `version` is 1 to 32 letters, digits,
`.`, `_`, `+` or `-`. Titles are at most 120 characters, descriptions 1000, `longDescription` 20 000.
`ownershipConfirmed` must be `true`. The server computes the SHA-256 itself; when the client sent `sha256` the two
must match (`checksum_mismatch` otherwise, and the upload is discarded). Sessions expire after 24 hours and are
purged, with their bytes, on every `POST /mods/uploads`. A creator may start `MOD_UPLOADS_PER_DAY` uploads per 24
hours (`429 rate_limited` with `Retry-After`) and hold `MOD_USER_QUOTA_BYTES` in total (`quota_exceeded`).
Files are never executed or extracted by the server.

### Endpoints

| Method and path | Auth | Purpose |
|---|---|---|
| `POST /mods/uploads` | user | Start an upload. `201 {uploadId, chunkSize, received, expiresAt}` |
| `PUT /mods/uploads/{id}/chunks?offset=N` | owner | Raw bytes (body limit 8 MiB). `offset` must equal `received`, else `409 offset_mismatch` with `error.received`. Bytes past the declared size: `400`. `200 {received}` |
| `GET /mods/uploads/{id}` | owner | Resume info `{received, size, state}` (`uploading` or `ready`) |
| `POST /mods/uploads/{id}/screenshots` | owner | Raw PNG, JPEG or WebP (magic bytes), at most 5 MiB and 8 per upload. `201 {index}` |
| `POST /mods/uploads/{id}/complete` | owner | Verifies the bytes and creates the version as `pending`. `201` version |
| `DELETE /mods/uploads/{id}` | owner | Abort; deletes the bytes. `204` |
| `GET /mods/mine` | user | `{mods: [{id, versions: [...]}]}`: every version with `status` and `rejectionReason` |
| `GET /admin/mods/pending` | admin | `{versions: [...]}` waiting for review |
| `POST /admin/mods/{mod}/versions/{version}/approve` | admin | `200 {status: "approved"}`; the file moves to the public area |
| `POST /admin/mods/{mod}/versions/{version}/reject` | admin | Body `{reason}`; `200 {status: "rejected"}`; the file is deleted, the row stays |
| `GET /catalog` | public | `{schemaVersion: 1, entries: [...]}` |
| `GET /mods/{id}` | public | Minimal escaped HTML page of an approved mod (the entry's `pageUrl`) |
| `GET /files/{mod}/{version}/{file}` | public | Approved files only, with `Range` (`206`, `416`, `Accept-Ranges`) |

Admins are the users whose Discord id is in `ADMIN_DISCORD_IDS`; anyone else gets `403 forbidden`. Another user's
upload answers `403 forbidden`. Chunk requests skip the per-IP rate limit (a big file is hundreds of chunks) but the
per-user limit still applies.

A version JSON looks like:

```json
{
  "modId": "brazil-livery", "version": "1.0", "title": "Brazil Livery", "description": "...",
  "longDescription": null, "category": "liveries", "game": "MotoGP 26", "fileName": "brazil-livery.zip",
  "size": 1234, "sha256": "<64 hex>", "status": "pending", "rejectionReason": null, "screenshotCount": 2,
  "uploadedAt": "2026-11-26T10:00:00Z", "reviewedAt": null
}
```

Every version stays as history. Uploading a version number that was **rejected** replaces the rejected row; any other
existing number answers `409 version_exists`. A mod id belongs to its first creator (`409 mod_id_taken` for others).

### `GET /catalog`

The latest **approved** version of every mod, in the same entry shape as `catalog/catalog.json`: `id`, `title`,
`author` (the creator's display name), `game`, `category`, `description`, `version`, `updatedAt` (approval date),
`pageUrl` (`{PUBLIC_BASE_URL}/mods/{id}`), `thumbnailUrl` and `screenshots` (first and all public screenshots),
`longDescription` and `download {url, size, sha256}` where `url` is
`{PUBLIC_BASE_URL}/files/{mod}/{version}/{fileName}` (percent-encoded). The response carries a strong `ETag`; a request
with a matching `If-None-Match` answers `304`. Pending and rejected versions never appear.

### How the launcher uses it

The launcher merges `GET /catalog` into the GitHub catalog (`MergedCatalogSource`): on the same id the community entry
wins, and when the community server cannot be reached or answers something unreadable (including a newer
`schemaVersion`) the launcher shows the GitHub catalog alone. Publishing goes through the endpoints above (8 MiB
chunks, resuming from `GET /mods/uploads/{id}` after a network error, up to 3 retries per chunk).

**Debug builds and `http://127.0.0.1:8787`:** the launcher's catalog domain only accepts `https` URLs (page, thumbnail,
screenshots and download). A debug launcher talking to a local plain-HTTP server therefore **skips every community
catalog entry** (they are counted as skipped, not shown). Publishing and "My mods" work against the local server, but
installing a published mod end to end needs an HTTPS deployment (`PUBLIC_BASE_URL` with `https`).

### Downloads

`GET /files/...` serves only the `public/` area of `MOD_STORAGE_DIR` (`tower-http` `ServeDir`), so pending and
incoming files cannot be reached, and path traversal is refused. In production Caddy serves the same directory
directly (see the deployment guide); the Rust route is the fallback and what the tests use.

## Adding routes in later issues

1. Write the use case in `community-application` against the ports in `ports.rs`; add new ports there when needed.
2. Implement the port in `community-infrastructure` (SQLite) and add a migration in `server/migrations/`
   (numbered `000N_name.sql`, never edit an applied migration).
3. Add a handler module in `server/api/src/` and register its router in `routes::extra_routes()`. The global IP
   limiter already covers it.
4. For authenticated routes, resolve the session in the `AuthenticatedUser` extractor and call
   `AppState::check_user_limit(&user_id)` so the per-user limit applies. Return `ApiError(AppError::...)` for every
   failure so the error format stays uniform.
5. Test through `build_router(state)` with `tower::ServiceExt::oneshot` and `community_api::testing::FakeClock`;
   tests must not use the real network or real time.
