#!/usr/bin/env bash
set -euo pipefail

# Install/update Project Zomboid dedicated server via SteamCMD on remote host.
# Usage: ./scripts/sync-zomboid.sh [--clean] <steam_username> [beta_branch]
# Run once initially and whenever PZ server needs updating.
#
# PZ dedicated server (app 380870) requires a Steam login (anonymous won't work).
# On first run SteamCMD will prompt for password and Steam Guard code interactively.
# Credentials are cached on the server after first successful login.
#
# Examples:
#   ./scripts/sync-zomboid.sh myuser                # unstable (Build 42, default)
#   ./scripts/sync-zomboid.sh myuser public          # stable branch (Build 41)
#   ./scripts/sync-zomboid.sh --clean myuser         # wipe + reinstall (for branch switch)
#
# Prerequisites:
#   - scripts/setup-server.sh has been run (pzuser exists, directories created)
#
# === PZ Server Directory Structure ===
#
#   /home/pzuser/pzserver/                   PZ dedicated server (managed by this script)
#   /home/pzuser/Zomboid/
#     Server/
#       zomboid-seasons.ini                  Main server config
#       zomboid-seasons_SandboxVars.lua      Gameplay tuning (zombies, loot, day length, etc.)
#       zomboid-seasons_spawnpoints.lua      Spawn locations
#       zomboid-seasons_spawnregions.lua     Spawn regions
#     Saves/Multiplayer/zomboid-seasons/     World save data (wiped on season rotation)
#
# === Key server.ini Settings ===
#
#   PublicName=           Server name in browser (managed by rotation code)
#   PublicDescription=    Server description
#   MaxPlayers=           Player cap
#   PVP=true/false        PvP toggle
#   Open=true/false       Show in public server list
#   Password=             Join password (empty = no password)
#   PauseEmpty=true       Pause when no players connected
#   Map=Muldraugh, KY    Map name
#   DefaultPort=16261     Game port (UDP)
#   SteamPort1=8766       Steam networking port
#   SteamPort2=8767       Steam networking port
#   RCONPort=27016        RCON port (configured by setup-zomboid-service.sh)
#   RCONPassword=         RCON password (configured by setup-zomboid-service.sh)
#
# === Mod Installation ===
#
#   Two fields in the .ini must be kept in sync (semicolon-separated):
#     WorkshopItems=steamId1;steamId2;steamId3
#     Mods=modId1;modId2;modId3
#
#   WorkshopItems = numeric Steam Workshop IDs (from the mod's Steam URL)
#   Mods          = string mod IDs (from each mod's mod.info file)
#   Server auto-downloads workshop mods on start; clients download when connecting.
#
# === SandboxVars.lua ===
#
#   Location: /home/pzuser/Zomboid/Server/zomboid-seasons_SandboxVars.lua
#   Controls gameplay: zombie population, loot respawn, day length, erosion, infection, etc.
#   Generated on first server start with defaults. Edit while server is stopped.
#   Delete the file to reset to defaults (PZ regenerates it).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../.env"

SERVER="${DEPLOY_SERVER}"
SSH_PORT="${DEPLOY_SSH_PORT}"
SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"

CLEAN=false
while [[ "${1:-}" == --* ]]; do
    case "$1" in
        --clean) CLEAN=true; shift ;;
        *) echo "Unknown flag: $1"; exit 1 ;;
    esac
done

STEAM_USER="${1:-}"
BETA_BRANCH="${2:-unstable}"
PZUSER="pzuser"
PZ_INSTALL_DIR="/home/${PZUSER}/pzserver"
PZ_APP_ID="380870"

if [ -z "$STEAM_USER" ]; then
    echo "Usage: ./scripts/sync-zomboid.sh [--clean] <steam_username> [beta_branch]"
    echo "PZ dedicated server requires Steam login (anonymous does not work)."
    echo "Default branch: unstable (Build 42)"
    echo ""
    echo "Flags:"
    echo "  --clean   Wipe install dir before downloading (needed when switching branches)"
    exit 1
fi

BETA_FLAG="-beta ${BETA_BRANCH}"
echo "==> Branch: ${BETA_BRANCH}"

echo "==> Ensuring 32-bit libraries and SteamCMD..."
$SSH_CMD "
dpkg --configure -a
dpkg --add-architecture i386
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq software-properties-common > /dev/null
add-apt-repository -y multiverse > /dev/null 2>&1 || true
echo 'steam steam/question select I AGREE' | debconf-set-selections
echo 'steam steam/license note ' | debconf-set-selections
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq lib32gcc-s1 steamcmd > /dev/null
echo 'SteamCMD ready.'
"

echo "==> Stopping zomboid.service (if running)..."
$SSH_CMD "systemctl is-active --quiet zomboid.service && systemctl stop zomboid.service && echo 'Stopped.' || echo 'Not running, skipping.'"

if [ "$CLEAN" = true ]; then
    echo "==> Wiping install dir (--clean)..."
    $SSH_CMD "rm -rf ${PZ_INSTALL_DIR} && echo 'Cleared ${PZ_INSTALL_DIR}'"
fi

echo "==> Installing/updating PZ dedicated server (app ${PZ_APP_ID})..."
echo "    Steam user: ${STEAM_USER} (will prompt for password/Steam Guard on first run)"
$SSH_CMD -t "
STEAMCMD=\$(command -v steamcmd || echo /usr/games/steamcmd)
if [ ! -x \"\$STEAMCMD\" ]; then
    echo 'ERROR: steamcmd not found. apt install may have failed.' >&2
    exit 1
fi
sudo -u ${PZUSER} \"\$STEAMCMD\" \
    +force_install_dir ${PZ_INSTALL_DIR} \
    +login ${STEAM_USER} \
    +app_update ${PZ_APP_ID} ${BETA_FLAG} validate \
    +quit"

echo "==> Fixing permissions..."
$SSH_CMD "chmod +x ${PZ_INSTALL_DIR}/start-server.sh && chown -R ${PZUSER}:${PZUSER} ${PZ_INSTALL_DIR}"

echo "==> Starting zomboid.service..."
$SSH_CMD "
if [ -f /etc/systemd/system/zomboid.service ]; then
    systemctl start zomboid.service
    sleep 2
    systemctl status zomboid.service --no-pager
else
    echo 'WARNING: zomboid.service not found. Run setup-zomboid-service.sh first.'
fi
"

echo "==> Sync complete."
