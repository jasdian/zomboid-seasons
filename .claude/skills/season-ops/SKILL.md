---
name: season-ops
description: "Season management: rotate seasons, archive saves, manage registrations."
disable-model-invocation: true
---

# Season Ops — Server Season Management

Manage Project Zomboid server seasons. Parse `$ARGUMENTS` for the operation.

## Operations

### `rotate` — Manual Season Rotation
Trigger the full season rotation sequence:
1. Save game via RCON: `save`
2. Stop server via RCON: `quit`
3. Wait for server to shut down
4. Archive current save directory as `.tar.gz`
5. Call the admin rotate endpoint: `POST /api/admin/rotate`
6. Restart PZ server: `sudo systemctl restart zomboid.service`
7. Verify server is running and new season is active

**Always confirm with the user before executing.**

### `status` — Season Status
Query current season info:
- `GET /api/season` — current season details
- Days remaining in current season
- Registration count and whitelist status

### `archive` — Archive Without Rotating
Archive the current season save without triggering rotation:
1. Save game via RCON
2. Archive save directory: `~/Zomboid/Saves/Multiplayer/<server_name>/`
3. Report archive location and size

### `whitelist` — Current Whitelist
List all whitelisted (confirmed) players for the current season:
- Query registrations with status = Confirmed
- Show username, registration date, payment method (ETH/promo)

## Server Details (from CLAUDE.md)
- RCON for game commands: `adduser`, `removeuserfromwhitelist`, `save`, `quit`
- Config: `~/Zomboid/Server/<server_name>.ini`
- Saves: `~/Zomboid/Saves/Multiplayer/<server_name>/`
- Service: `zomboid.service` (managed via systemd)

## Rules
- Always confirm destructive operations (rotate, archive) with the user
- Show current state before making changes
- Report results after each step
