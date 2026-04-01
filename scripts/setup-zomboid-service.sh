#!/usr/bin/env bash
set -euo pipefail

# Create zomboid.service + zomboid.socket systemd units for PZ dedicated server
# Uses a FIFO pipe for stdin (required for admin password prompt and server commands)
# Usage: ./scripts/setup-zomboid-service.sh
#
# Prerequisites:
#   - scripts/setup-server.sh has been run (pzuser exists)
#   - scripts/sync-zomboid.sh has been run (PZ server installed at /home/pzuser/pzserver/)
#   - /root/zomboid-seasons/rcon.pw exists on the server
#   - First run completed (admin account created) — if not, run:
#       ssh -p $DEPLOY_SSH_PORT $DEPLOY_SERVER
#       ADMIN_PW=$(cat /root/zomboid-seasons/rcon.pw)
#       echo -e "${ADMIN_PW}\n${ADMIN_PW}" | sudo -u pzuser bash /home/pzuser/pzserver/start-server.sh -servername zomboid-seasons
#       # Wait for "Administrator account 'admin' created" then Ctrl+C

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../.env"

SERVER="${DEPLOY_SERVER}"
SSH_PORT="${DEPLOY_SSH_PORT}"
SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"

RCON_PW_FILE="/root/zomboid-seasons/rcon.pw"
SERVER_NAME="zomboid-seasons"
PZ_INSTALL_DIR="/home/pzuser/pzserver"

echo "==> Checking prerequisites..."
$SSH_CMD "
if [ ! -f ${RCON_PW_FILE} ]; then
    echo 'ERROR: ${RCON_PW_FILE} not found. Create it first:'
    echo '  openssl rand -hex 16 > ${RCON_PW_FILE}'
    exit 1
fi
if [ ! -f ${PZ_INSTALL_DIR}/start-server.sh ]; then
    echo 'ERROR: PZ server not installed. Run scripts/sync-zomboid.sh first.'
    exit 1
fi
"

echo "==> Writing zomboid.service..."
$SSH_CMD "
UNIT_FILE='/etc/systemd/system/zomboid.service'
if [ -f \"\$UNIT_FILE\" ]; then
    cp \"\$UNIT_FILE\" \"\${UNIT_FILE}.bak\"
fi

cat > \"\$UNIT_FILE\" <<'UNIT'
[Unit]
Description=Project Zomboid Dedicated Server
After=network.target

[Service]
PrivateTmp=true
Type=simple
User=pzuser
WorkingDirectory=/home/pzuser/pzserver
ExecStart=/bin/sh -c \"exec /home/pzuser/pzserver/start-server.sh -servername zomboid-seasons </home/pzuser/pzserver/zomboid.control\"
ExecStop=/bin/sh -c \"echo save > /home/pzuser/pzserver/zomboid.control; sleep 15; echo quit > /home/pzuser/pzserver/zomboid.control\"
Sockets=zomboid.socket
KillSignal=SIGCONT
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
UNIT
"

echo "==> Writing zomboid.socket..."
$SSH_CMD "
cat > /etc/systemd/system/zomboid.socket <<'UNIT'
[Unit]
BindsTo=zomboid.service

[Socket]
ListenFIFO=/home/pzuser/pzserver/zomboid.control
FileDescriptorName=control
RemoveOnStop=true
SocketMode=0660
SocketUser=pzuser
UNIT
"

echo "==> Configuring RCON in server ini..."
$SSH_CMD "
INI_DIR='/home/pzuser/Zomboid/Server'
mkdir -p \"\$INI_DIR\"
INI_FILE=\"\${INI_DIR}/${SERVER_NAME}.ini\"
RCON_PW=\$(cat ${RCON_PW_FILE})

if [ -f \"\$INI_FILE\" ]; then
    sed -i \"s/^RCONPort=.*/RCONPort=27016/\" \"\$INI_FILE\"
    sed -i \"s/^RCONPassword=.*/RCONPassword=\${RCON_PW}/\" \"\$INI_FILE\"
    echo 'Updated RCON settings in ini.'
else
    echo 'NOTE: ini not found yet. RCON settings will need to be configured after first server start.'
fi
"

echo "==> Reloading systemd and starting service..."
$SSH_CMD "
systemctl daemon-reload
systemctl enable zomboid.socket zomboid.service
systemctl start zomboid.socket
systemctl start zomboid.service
sleep 5
systemctl status zomboid.service --no-pager
"

echo "==> Done. PZ server with RCON on port 27016."
echo "    Send commands: echo 'help' > /home/pzuser/pzserver/zomboid.control"
