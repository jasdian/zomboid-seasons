#!/usr/bin/env bash
set -euo pipefail

# One-time server setup: create pzuser, directories, firewall rules
# Usage: ./scripts/setup-server.sh

SERVER="root@xxx.xxx.xxx.xxx"
SSH_PORT="55555"
SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"

echo "==> Creating directories..."
$SSH_CMD "mkdir -p /root/zomboid-seasons"

echo "==> Creating pzuser for PZ server..."
$SSH_CMD "
id -u pzuser &>/dev/null || useradd -m -s /bin/bash pzuser
mkdir -p /home/pzuser/Zomboid/Saves/Multiplayer
mkdir -p /home/pzuser/Zomboid/Server
chown -R pzuser:pzuser /home/pzuser
"

echo "==> Configuring UFW firewall..."
$SSH_CMD "
apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq ufw > /dev/null
ufw default deny incoming
ufw default allow outgoing
ufw allow 55555/tcp comment 'SSH'
ufw allow 80/tcp comment 'HTTP'
ufw allow 443/tcp comment 'HTTPS'
ufw allow 16261/udp comment 'PZ game port'
ufw allow 16262 comment 'PZ Steam query port'
ufw allow from 172.16.0.0/12 to any port 27016 proto tcp comment 'RCON from Docker'
ufw --force enable
ufw status verbose
"

echo "==> Server setup complete."
