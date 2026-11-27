#!/usr/bin/env bash
# Runs the community server in local dev mode: fake Discord sign-in, demo data,
# throwaway SQLite database and mod storage under server/.dev-data.
# The launcher's debug build already points at http://127.0.0.1:8787.
#
#   scripts/dev-server.sh            # start (seeded)
#   DEV_FAKE_USER=ana scripts/dev-server.sh
#
# Never use these variables on a real server: the server refuses DEV_FAKE_AUTH
# unless PUBLIC_BASE_URL is a loopback URL.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
data="$root/server/.dev-data"
# Native Windows binaries need a Windows-style path when run from Git Bash.
if command -v cygpath >/dev/null 2>&1; then data="$(cygpath -m "$data")"; fi
mkdir -p "$data/mods"

export DEV_FAKE_AUTH=1
export DEV_SEED=1
export PUBLIC_BASE_URL="http://127.0.0.1:8787"
export BIND_ADDR="127.0.0.1:8787"
export DATABASE_URL="sqlite:$data/community.db"
export MOD_STORAGE_DIR="$data/mods"
# The default fake user is an admin, so the admin web works out of the box.
export DEV_FAKE_DISCORD_ID="${DEV_FAKE_DISCORD_ID:-900000000000000001}"
export ADMIN_DISCORD_IDS="${ADMIN_DISCORD_IDS:-$DEV_FAKE_DISCORD_ID}"

cd "$root/server"
exec cargo run -p community-api --bin motogp-community
