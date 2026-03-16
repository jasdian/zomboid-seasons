#!/usr/bin/env bash
set -euo pipefail

# Reset the production database with a clean schema.
# 1. Backs up registrations + promo_codes from the running DB
# 2. Stops the container
# 3. Replaces migrations/ with flattened single-file schema
# 4. Deletes the DB volume
# 5. Restarts the container (migrations recreate clean DB)
# 6. Re-imports backed-up registrations + promo_codes
#
# Player stats (kills) are NOT backed up — they re-accumulate
# naturally from kills.json as players get kills.
#
# Usage: nix-shell --run "./scripts/reset-db.sh"

SERVER="root@xxx.xxx.xxx.xxx"
SSH_PORT="55555"
REMOTE_DIR="/root/zomboid-seasons"
# DB lives on a Docker volume; find it via the host filesystem
DB_VOLUME="/var/lib/docker/volumes/zomboid-seasons_db-data/_data/seasons.db"

SSH_CMD="ssh -p ${SSH_PORT} ${SERVER}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="${REMOTE_DIR}/backups/db_reset_${TIMESTAMP}"

echo "==> Creating backup directory..."
$SSH_CMD "mkdir -p ${BACKUP_DIR}"

echo "==> Backing up registrations and promo_codes..."
$SSH_CMD "
which sqlite3 >/dev/null 2>&1 || apt-get install -y sqlite3
if [ -f ${DB_VOLUME} ]; then
    sqlite3 ${DB_VOLUME} '.dump registrations' > ${BACKUP_DIR}/registrations.sql 2>/dev/null || true
    sqlite3 ${DB_VOLUME} '.dump promo_codes' > ${BACKUP_DIR}/promo_codes.sql 2>/dev/null || true
    sqlite3 ${DB_VOLUME} '.dump seasons' > ${BACKUP_DIR}/seasons.sql 2>/dev/null || true
else
    echo 'No existing DB found, skipping backup'
fi
echo 'Backup saved to ${BACKUP_DIR}/'
ls -la ${BACKUP_DIR}/
"

echo "==> Stopping container..."
$SSH_CMD "cd ${REMOTE_DIR} && docker compose down"

echo "==> Replacing migrations with flattened schema..."
$SSH_CMD "rm -f ${REMOTE_DIR}/migrations/*.sql"
scp -P ${SSH_PORT} migrations_flat/001_initial.sql "${SERVER}:${REMOTE_DIR}/migrations/001_initial.sql"

echo "==> Deleting database..."
$SSH_CMD "
docker volume rm zomboid-seasons_db-data 2>/dev/null || true
rm -f ${DB_VOLUME} ${DB_VOLUME}-wal ${DB_VOLUME}-shm 2>/dev/null || true
"

echo "==> Rebuilding and starting container (migrations create fresh DB)..."
$SSH_CMD "cd ${REMOTE_DIR} && docker compose up -d --build"

echo "==> Waiting for backend to become healthy..."
for i in $(seq 1 30); do
    STATUS=$($SSH_CMD "docker inspect --format='{{.State.Health.Status}}' zomboid-seasons 2>/dev/null" || echo "starting")
    if [ "$STATUS" = "healthy" ]; then
        echo "==> Backend is healthy."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: Backend did not become healthy."
        $SSH_CMD "cd ${REMOTE_DIR} && docker compose logs --tail 30"
        exit 1
    fi
    sleep 2
done

echo "==> Re-importing backed-up data..."
$SSH_CMD "
DB=${DB_VOLUME}

# Import seasons first (registrations have FK to seasons)
if [ -f ${BACKUP_DIR}/seasons.sql ] && grep -q '^INSERT' ${BACKUP_DIR}/seasons.sql 2>/dev/null; then
    grep '^INSERT' ${BACKUP_DIR}/seasons.sql | sqlite3 \$DB 2>&1 || echo 'Seasons: skipped (already seeded)'
    echo 'Seasons imported'
fi

if [ -f ${BACKUP_DIR}/registrations.sql ] && grep -q '^INSERT' ${BACKUP_DIR}/registrations.sql 2>/dev/null; then
    grep '^INSERT' ${BACKUP_DIR}/registrations.sql | sqlite3 \$DB 2>&1 || echo 'Registrations import error'
    echo 'Registrations imported'
fi

if [ -f ${BACKUP_DIR}/promo_codes.sql ] && grep -q '^INSERT' ${BACKUP_DIR}/promo_codes.sql 2>/dev/null; then
    grep '^INSERT' ${BACKUP_DIR}/promo_codes.sql | sqlite3 \$DB 2>&1 || echo 'Promo codes import error'
    echo 'Promo codes imported'
fi
"

echo "==> Verifying imported data..."
$SSH_CMD "
DB=${DB_VOLUME}
sqlite3 \$DB 'SELECT count(*) || \" seasons\" FROM seasons;'
sqlite3 \$DB 'SELECT count(*) || \" registrations\" FROM registrations;'
sqlite3 \$DB 'SELECT count(*) || \" promo_codes\" FROM promo_codes;'
sqlite3 \$DB 'SELECT count(*) || \" player_stats (should be 0)\" FROM player_stats;'
"

echo "==> DB reset complete."
echo "    Backup at: ${BACKUP_DIR}/"
echo "    Player kills will re-accumulate from kills.json automatically."
