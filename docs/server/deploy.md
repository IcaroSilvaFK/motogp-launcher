# Community server: deployment

The community server (`server/`) is a single Rust binary, `motogp-community`, backed by SQLite. It listens on
`127.0.0.1:8787` and sits behind Caddy, which terminates HTTPS. These steps assume a Debian/Ubuntu VPS with a
domain name pointing at it (`community.example.com` below).

## 1. Build

On a build machine (or the VPS) with the Rust toolchain:

```sh
cd server
cargo build --release
# binary: server/target/release/motogp-community (or your configured target dir)
```

Copy the binary to the VPS as `/usr/local/bin/motogp-community`. Migrations are embedded in the binary and run at
startup, so no extra files are needed.

## 2. User and directories

```sh
sudo useradd --system --home /var/lib/motogp-community --shell /usr/sbin/nologin motogp-community
sudo install -d -o motogp-community -g motogp-community -m 0750 /var/lib/motogp-community
sudo install -d -o motogp-community -g motogp-community -m 0750 /var/backups/motogp-community
# Mod files (uploads in progress, pending review, public). The service creates the subfolders itself.
sudo install -d -o motogp-community -g motogp-community -m 0755 /var/lib/motogp-community/mods
```

**Disk space planning.** Mod files live under `MOD_STORAGE_DIR`. Budget for the worst case: a user may hold
`MOD_USER_QUOTA_BYTES` (default 10 GiB) and an upload may be `MOD_MAX_UPLOAD_BYTES` (default 2 GiB), so the number of
active creators times the quota is the ceiling, plus one cap of temporary space per upload in progress (`incoming/`
bytes are purged after 24 hours when abandoned). Lower the two limits, or put the folder on a bigger volume, before
opening publishing to many creators. If `MOD_STORAGE_DIR` is outside `/var/lib/motogp-community`, add its path to
`ReadWritePaths` in the systemd unit below. The `public/` folder must be readable by Caddy (mode `0755`, which the
command above uses; the files the service writes inherit the default umask `0644`).

## 3. Discord application

1. Open <https://discord.com/developers/applications> and create a **New Application**.
2. Under **OAuth2**, copy the **Client ID** (also shown under General Information) and click **Reset Secret** to
   get the **Client Secret**. The secret is shown once; store it only in the env file below.
3. Under **OAuth2 > Redirects**, add exactly `https://community.example.com/auth/discord/callback`.
4. The server only requests the `identify` scope. Nothing else needs to be enabled.

## 4. Environment file

Create `/etc/motogp-community.env`, readable only by root and the service:

```sh
sudo install -m 0600 -o root -g root /dev/null /etc/motogp-community.env
sudoedit /etc/motogp-community.env
```

```ini
DISCORD_CLIENT_ID=<application id>
DISCORD_CLIENT_SECRET=<client secret>
PUBLIC_BASE_URL=https://community.example.com
DATABASE_URL=sqlite:///var/lib/motogp-community/community.db
ADMIN_DISCORD_IDS=<your discord user id>,<another id>
MOD_STORAGE_DIR=/var/lib/motogp-community/mods
BIND_ADDR=127.0.0.1:8787
TRUST_PROXY_HEADERS=true
# Optional mod hosting limits (defaults shown):
# MOD_MAX_UPLOAD_BYTES=2147483648
# MOD_USER_QUOTA_BYTES=10737418240
# MOD_UPLOADS_PER_DAY=10
```

| Variable | Default | Meaning |
|----------|---------|---------|
| `MOD_STORAGE_DIR` | required | Root of the mod files: `incoming/`, `pending/` and `public/` |
| `MOD_MAX_UPLOAD_BYTES` | 2147483648 (2 GiB) | Largest file; a bigger declared size is refused before storing |
| `MOD_USER_QUOTA_BYTES` | 10737418240 (10 GiB) | Total a user may hold (non-rejected versions plus uploads in progress) |
| `MOD_UPLOADS_PER_DAY` | 10 | Uploads a user may start per 24 hours |

`TRUST_PROXY_HEADERS=true` makes the server read the client IP from the right-most `X-Forwarded-For` value, which
Caddy appends. Set it **only** when the server is reachable exclusively through Caddy (it binds to loopback, so it
is). Otherwise clients could spoof their IP and dodge the rate limits.

The service runs as `motogp-community`, so it cannot read a root-only file directly: systemd reads
`EnvironmentFile` as root before dropping privileges, so `0600 root:root` works.

If a required variable is missing, the server exits at startup with a message naming the variable (never its
value). See `server/.env.example` for the full list.

## 5. systemd unit

`/etc/systemd/system/motogp-community.service`:

```ini
[Unit]
Description=MotoGP Launcher community server
After=network-online.target
Wants=network-online.target

[Service]
User=motogp-community
Group=motogp-community
EnvironmentFile=/etc/motogp-community.env
ExecStart=/usr/local/bin/motogp-community
Restart=on-failure
RestartSec=3
Environment=RUST_LOG=info

# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/motogp-community

[Install]
WantedBy=multi-user.target
```

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now motogp-community
sudo systemctl status motogp-community
curl -s http://127.0.0.1:8787/health   # {"status":"ok"}
```

The server shuts down gracefully on `SIGTERM` (`systemctl stop`), finishing in-flight requests.

## 6. Caddy (automatic HTTPS)

Install Caddy from its official package repository, then `/etc/caddy/Caddyfile`:

```caddyfile
community.example.com {
    # Approved mod files are served straight from disk (fast, with Range support).
    # Keep this before reverse_proxy. Only the public/ folder is exposed.
    handle_path /files/* {
        root * /var/lib/motogp-community/mods/public
        file_server
    }
    reverse_proxy 127.0.0.1:8787
}
```

Large uploads come through `reverse_proxy` in 8 MiB chunks, so no special body limit is needed in Caddy. The Rust
server also serves `/files/*` itself, so everything still works when Caddy is not serving the folder.

```sh
sudo systemctl reload caddy
```

Caddy obtains and renews the Let's Encrypt certificate automatically; ports 80 and 443 must be open. Verify with
`curl https://community.example.com/health`.

## 7. Backups

SQLite runs in WAL mode, so copy the file with the online backup command (never plain `cp`):

```sh
sudo -u motogp-community sqlite3 /var/lib/motogp-community/community.db \
  ".backup '/var/backups/motogp-community/community-$(date +%F).db'"
```

Schedule it daily with cron or a systemd timer and prune old files, for example:

```cron
15 3 * * * motogp-community sqlite3 /var/lib/motogp-community/community.db ".backup '/var/backups/motogp-community/community-$(date +\%F).db'" && find /var/backups/motogp-community -name 'community-*.db' -mtime +14 -delete
```

Restore by stopping the service and copying a backup over `community.db`.

The mod files are not in the database: back up `MOD_STORAGE_DIR` next to the SQLite backup, otherwise a restored
database points at files that no longer exist. `rsync` copies only what changed:

```sh
sudo rsync -a --delete /var/lib/motogp-community/mods/ /var/backups/motogp-community/mods/
```

`incoming/` holds partial uploads and may be skipped (`--exclude incoming/`). Restore the database and the folder from
the same moment when you can.

## 7b. Moderation and approving mods (the admin page)

Creators publish from the launcher; every version waits for an admin. Admins also handle reported comments and bans.
All of it lives in the admin page:

1. Put your Discord user id in `ADMIN_DISCORD_IDS` (comma-separated) in `/etc/motogp-community/env`
   (Discord: Settings > Advanced > Developer Mode, then right-click your name > Copy User ID).
2. Restart the service: `sudo systemctl restart motogp-community`.
3. Open `https://community.example.com/admin` and press "Sign in with Discord". The Discord application is the same
   as the launcher's and the redirect URI stays `https://community.example.com/auth/discord/callback`: nothing to
   change in the Discord developer portal.
4. The page lists open reports, versions waiting for review (metadata, screenshots, a download link, Approve or
   Reject with a reason), hidden comments, banned users and the last 50 actions. The session lasts 12 hours.

The admin cookie is `Secure`, so the page needs HTTPS (Caddy provides it); browsers also accept it on
`http://localhost` for a local run. Caddy needs no change: only `/files/*` is handled by `file_server`, and
`/admin`, `/auth/*` and everything else already go through `reverse_proxy`, which passes cookies and the redirect
unchanged. To remove an admin, delete the id from `ADMIN_DISCORD_IDS` and restart: their session stops working at
once.

### Fallback: curl

If the page is not available, the JSON routes still work with the session token of an admin (the launcher stores
its token, or sign in once and read it from `POST /auth/token`):

```sh
TOKEN=<admin session token>
BASE=https://community.example.com

# What is waiting?
curl -s -H "Authorization: Bearer $TOKEN" $BASE/admin/mods/pending

# Approve (the file becomes public) or reject (the file is deleted, the creator sees the reason).
curl -s -X POST -H "Authorization: Bearer $TOKEN" $BASE/admin/mods/brazil-livery/versions/1.0/approve
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"reason":"Contains assets we cannot redistribute"}' \
  $BASE/admin/mods/brazil-livery/versions/1.0/reject
```

Pending files sit in `pending/` and are never served; only `public/` is exposed.

## 8. Updating

```sh
sudo systemctl stop motogp-community
sudo install -m 0755 motogp-community /usr/local/bin/motogp-community
sudo systemctl start motogp-community
```

Migrations run automatically on start. Take a backup first.
