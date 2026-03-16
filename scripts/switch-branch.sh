#!/usr/bin/env bash
set -euo pipefail

# Switch PZ server between branches. Wipes install dir, saves, and server configs,
# then reinstalls and redeploys.
#
# Usage: ./scripts/switch-branch.sh <steam_username> [beta_branch]
#
# Examples:
#   ./scripts/switch-branch.sh myuser              # switch to default (unstable/Build 42)
#   ./scripts/switch-branch.sh myuser public        # switch to stable (Build 41)

STEAM_USER="${1:-}"
BETA_BRANCH="${2:-}"

if [ -z "$STEAM_USER" ]; then
    echo "Usage: ./scripts/switch-branch.sh <steam_username> [beta_branch]"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

SERVER="root@xxx.xxx.xxx.xxx"
SSH_PORT="55555"
SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"

echo "==> Step 1/5: Wipe B42 save data and server configs..."
$SSH_CMD 'rm -rf /home/pzuser/Zomboid/Saves/Multiplayer/zomboid-seasons /home/pzuser/Zomboid/Server/zomboid-seasons*.ini /home/pzuser/Zomboid/Server/zomboid-seasons*.lua'
echo "    Done."

echo "==> Step 2/5: Wipe install dir and download PZ server..."
BRANCH_ARGS=""
if [ -n "$BETA_BRANCH" ]; then
    BRANCH_ARGS="$BETA_BRANCH"
fi
"${SCRIPT_DIR}/sync-zomboid.sh" --clean "$STEAM_USER" $BRANCH_ARGS

echo "==> Step 3/5: First start to generate fresh configs..."
$SSH_CMD '
systemctl start zomboid.service
echo "    Waiting 30s for config generation..."
sleep 30
systemctl stop zomboid.service
echo "    Done."
'

echo "==> Step 4/5: Re-apply RCON settings..."
"${SCRIPT_DIR}/setup-zomboid-service.sh"

echo "==> Step 5/5: Deploy updated backend..."
"${SCRIPT_DIR}/deploy.sh"

echo "==> Branch switch complete. Server is running."
