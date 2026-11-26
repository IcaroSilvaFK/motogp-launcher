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
| `not_found` | 404 | Unknown route or resource |
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
