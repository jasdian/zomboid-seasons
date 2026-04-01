#!/usr/bin/env bash
set -euo pipefail

# Deploy zomboid-status service to production server
# Usage: nix-shell --run ./scripts/deploy-status.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../.env"

SERVER="${DEPLOY_SERVER}"
SSH_PORT="${DEPLOY_SSH_PORT}"
REMOTE_DIR="${DEPLOY_REMOTE_DIR}"
BINARY_NAME="zomboid-status"

SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"
SCP_CMD="scp -P ${SSH_PORT}"

echo "==> Building zomboid-status release binary (musl static)..."
cargo build --release -p zomboid-status --target x86_64-unknown-linux-musl

BINARY="target/x86_64-unknown-linux-musl/release/${BINARY_NAME}"
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    exit 1
fi

echo "==> Binary size: $(du -h "$BINARY" | cut -f1)"

echo "==> Creating remote directories..."
$SSH_CMD "mkdir -p ${REMOTE_DIR}/status-service/{static,migrations}"

echo "==> Copying files to server..."
$SCP_CMD "$BINARY" "${SERVER}:${REMOTE_DIR}/${BINARY_NAME}"
$SCP_CMD status-service/Dockerfile "${SERVER}:${REMOTE_DIR}/status-service/Dockerfile"
$SCP_CMD compose.yml "${SERVER}:${REMOTE_DIR}/compose.yml"
$SCP_CMD status-config.prod.toml "${SERVER}:${REMOTE_DIR}/status-config.prod.toml"
$SSH_CMD "chmod 600 ${REMOTE_DIR}/status-config.prod.toml"
rsync -avz -e "ssh -p ${SSH_PORT}" status-service/static/ "${SERVER}:${REMOTE_DIR}/status-service/static/"
rsync -avz -e "ssh -p ${SSH_PORT}" status-service/migrations/ "${SERVER}:${REMOTE_DIR}/status-service/migrations/"

echo "==> Building and restarting status container..."
$SSH_CMD "cd ${REMOTE_DIR} && docker compose up -d --build status"

echo "==> Waiting for status service to become healthy..."
for i in $(seq 1 30); do
    STATUS=$($SSH_CMD "docker inspect --format='{{.State.Health.Status}}' zomboid-status 2>/dev/null" || echo "starting")
    if [ "$STATUS" = "healthy" ]; then
        echo "==> Status service is healthy."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: Status service did not become healthy after 30 attempts."
        $SSH_CMD "cd ${REMOTE_DIR} && docker compose logs --tail 30 status"
        exit 1
    fi
    sleep 2
done

echo "==> Container logs:"
$SSH_CMD "cd ${REMOTE_DIR} && docker compose logs --tail 20 status"

echo "==> Status service deploy complete."
