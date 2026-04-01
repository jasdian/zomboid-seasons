#!/usr/bin/env bash
# Send RCON commands to the PZ server
# Usage: ./scripts/rcon.sh <command>
# Examples:
#   ./scripts/rcon.sh players
#   ./scripts/rcon.sh 'teleportto "BINDIN" 8071,11835,0'
#   ./scripts/rcon.sh 'adduser "player" "pass"'
#   ./scripts/rcon.sh help

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../.env"

SERVER="${DEPLOY_SERVER}"
SSH_PORT="${DEPLOY_SSH_PORT}"
RCON_PW_FILE="${DEPLOY_REMOTE_DIR}/rcon.pw"

if [ $# -eq 0 ]; then
    echo "Usage: $0 <rcon command>"
    echo "Examples:"
    echo "  $0 players"
    echo "  $0 'teleportto \"BINDIN\" 8071,11835,0'"
    echo "  $0 help"
    exit 1
fi

CMD="$*"

ssh -p "$SSH_PORT" "$SERVER" \
    "RCON_PW=\$(cat $RCON_PW_FILE) && docker run --rm --network host itzg/rcon-cli --host 127.0.0.1 --port 27016 --password \"\$RCON_PW\" $CMD"
