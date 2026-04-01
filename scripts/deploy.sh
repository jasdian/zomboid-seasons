#!/usr/bin/env bash
set -euo pipefail

# Deploy zomboid-seasons to production server (Docker-based)
# Usage: nix-shell --run ./scripts/deploy.sh
#        nix-shell --run "./scripts/deploy.sh --mod-only"  (hot-reload Lua only)
#        nix-shell --run "./scripts/deploy.sh --reset-db"  (backup + flatten + recreate DB)

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../.env"

SERVER="${DEPLOY_SERVER}"
SSH_PORT="${DEPLOY_SSH_PORT}"
REMOTE_DIR="${DEPLOY_REMOTE_DIR}"
BINARY_NAME="zomboid-seasons"

SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"
SCP_CMD="scp -P ${SSH_PORT}"

MOD_ONLY=false
RESET_DB=false
if [ "${1:-}" = "--mod-only" ]; then
    MOD_ONLY=true
elif [ "${1:-}" = "--reset-db" ]; then
    RESET_DB=true
fi

if [ "$RESET_DB" = true ]; then
    echo "==> Running DB reset (backup + flatten + recreate)..."
    exec "$(dirname "$0")/reset-db.sh"
fi

if [ "$MOD_ONLY" = false ]; then
    echo "==> Building release binary (musl static)..."
    cargo build --release -p zomboid-seasons --target x86_64-unknown-linux-musl

    BINARY="target/x86_64-unknown-linux-musl/release/${BINARY_NAME}"
    if [ ! -f "$BINARY" ]; then
        echo "ERROR: Binary not found at $BINARY"
        exit 1
    fi

    echo "==> Binary size: $(du -h "$BINARY" | cut -f1)"

    echo "==> Creating remote directories..."
    $SSH_CMD "mkdir -p ${REMOTE_DIR}/{static,migrations,scripts,backups}"

    echo "==> Copying files to server..."
    $SCP_CMD "$BINARY" "${SERVER}:${REMOTE_DIR}/${BINARY_NAME}"
    $SCP_CMD Dockerfile "${SERVER}:${REMOTE_DIR}/Dockerfile"
    $SCP_CMD compose.yml "${SERVER}:${REMOTE_DIR}/compose.yml"
    $SCP_CMD config.prod.toml "${SERVER}:${REMOTE_DIR}/config.prod.toml"
    $SSH_CMD "chmod 600 ${REMOTE_DIR}/config.prod.toml"
    rsync -avz -e "ssh -p ${SSH_PORT}" static/ "${SERVER}:${REMOTE_DIR}/static/"
    rsync -avz -e "ssh -p ${SSH_PORT}" migrations/ "${SERVER}:${REMOTE_DIR}/migrations/"
    rsync -avz -e "ssh -p ${SSH_PORT}" scripts/backup.sh "${SERVER}:${REMOTE_DIR}/scripts/backup.sh"
fi

echo "==> Setting up server-side Lua (deployed into PZ game dir, no Mods= needed)..."
# Lua files go directly into PZ's media/lua/server/ so they load automatically.
# PZ sandboxes getFileWriter/getFileReader under ~/Zomboid/Lua/,
# so data files (kills.json, drops_*.json) must live there.
PZ_SERVER_LUA="/home/pzuser/pzserver/media/lua/server/ZomboidSeasons"
$SSH_CMD "
mkdir -p ${PZ_SERVER_LUA}
mkdir -p /home/pzuser/Zomboid/Lua/ZomboidSeasons
[ -f /home/pzuser/Zomboid/Lua/ZomboidSeasons/kills.json ] || echo '{\"players\":[]}' > /home/pzuser/Zomboid/Lua/ZomboidSeasons/kills.json
[ -f /home/pzuser/Zomboid/Lua/ZomboidSeasons/drops_pending.json ] || echo '{\"commands\":[]}' > /home/pzuser/Zomboid/Lua/ZomboidSeasons/drops_pending.json
[ -f /home/pzuser/Zomboid/Lua/ZomboidSeasons/drops_claimed.json ] || echo '{\"claims\":[]}' > /home/pzuser/Zomboid/Lua/ZomboidSeasons/drops_claimed.json
[ -f /home/pzuser/Zomboid/Lua/ZomboidSeasons/admin_commands.json ] || echo '{\"commands\":[]}' > /home/pzuser/Zomboid/Lua/ZomboidSeasons/admin_commands.json
[ -f /home/pzuser/Zomboid/Lua/ZomboidSeasons/gametime.json ] || echo '{\"month\":null,\"day\":null,\"hour\":null,\"minute\":null}' > /home/pzuser/Zomboid/Lua/ZomboidSeasons/gametime.json
"
rsync -avz --delete -e "ssh -p ${SSH_PORT}" mod/ZomboidSeasons/media/lua/server/ZomboidSeasons/ "${SERVER}:${PZ_SERVER_LUA}/"
$SSH_CMD "chown -R pzuser:pzuser ${PZ_SERVER_LUA} /home/pzuser/Zomboid/Lua/ZomboidSeasons"

if [ "$MOD_ONLY" = true ]; then
    echo "==> Mod-only deploy: hot-reloading Lua via backend API..."
    ADMIN_TOKEN=$(grep -oP 'token = "\K[^"]+' config.prod.toml)
    curl -sf -X POST "https://zomboid.princeofcrypto.com/api/admin/reload-lua" \
        -H "Authorization: Bearer ${ADMIN_TOKEN}"
    echo ""
    echo "==> Hot-reload complete."
else
    echo "==> Setting up backup cron job..."
    $SSH_CMD "
    CRON_CMD='0 3 * * * ${REMOTE_DIR}/scripts/backup.sh ${REMOTE_DIR}/backups'
    (crontab -l 2>/dev/null | grep -v 'backup.sh' ; echo \"\$CRON_CMD\") | crontab -
    echo 'Cron job configured: daily 03:00 backup'
    "

    echo "==> Building and restarting backend container..."
    $SSH_CMD "cd ${REMOTE_DIR} && docker compose up -d --build backend"

    echo "==> Waiting for backend to become healthy..."
    for i in $(seq 1 30); do
        STATUS=$($SSH_CMD "docker inspect --format='{{.State.Health.Status}}' zomboid-seasons 2>/dev/null" || echo "starting")
        if [ "$STATUS" = "healthy" ]; then
            echo "==> Backend is healthy."
            break
        fi
        if [ "$i" -eq 30 ]; then
            echo "ERROR: Backend did not become healthy after 30 attempts."
            $SSH_CMD "cd ${REMOTE_DIR} && docker compose logs --tail 30 backend"
            exit 1
        fi
        sleep 2
    done

    echo "==> Container logs:"
    $SSH_CMD "cd ${REMOTE_DIR} && docker compose logs --tail 20 backend"

    echo "==> Hot-reloading Lua mod (no PZ restart)..."
    ADMIN_TOKEN=$(grep -oP 'token = "\K[^"]+' config.prod.toml)
    curl -sf -X POST "https://zomboid.princeofcrypto.com/api/admin/reload-lua" \
        -H "Authorization: Bearer ${ADMIN_TOKEN}" || echo "WARN: hot-reload failed (PZ server may not be running yet)"

    echo ""
    echo "==> Deploy complete. Run 'systemctl restart zomboid.service' manually if .ini changes need picking up."
fi
