#!/usr/bin/env bash
set -euo pipefail

# Restart the PZ game server via systemctl.
# Use after .ini changes, sandbox settings updates, or game updates.
# Usage: ./scripts/restart-pz.sh

SERVER="root@xxx.xxx.xxx.xxx"
SSH_PORT="55555"
SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"

echo "==> Restarting PZ game server..."
$SSH_CMD "systemctl restart zomboid.service"

echo "==> Waiting for PZ to start..."
sleep 10

echo "==> Service status:"
$SSH_CMD "systemctl status zomboid.service --no-pager -l" || true

echo "==> PZ server restarted."
