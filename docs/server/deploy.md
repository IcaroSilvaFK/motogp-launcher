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
```

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
BIND_ADDR=127.0.0.1:8787
TRUST_PROXY_HEADERS=true
```

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
    reverse_proxy 127.0.0.1:8787
}
```

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

## 8. Updating

```sh
sudo systemctl stop motogp-community
sudo install -m 0755 motogp-community /usr/local/bin/motogp-community
sudo systemctl start motogp-community
```

Migrations run automatically on start. Take a backup first.
