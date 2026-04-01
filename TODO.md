# TODO — Zomboid Seasons Features

## 1. Kill Leaderboard + Zombie Bounty System ✅

Track zombie kills per player per season. Top killers get rewards (free registration next season).

### Server-side Lua mod (`mod/ZomboidSeasons/media/lua/server/ZomboidSeasons/`)

- [x] Hook `Events.OnZombieDead` — identify killer, increment kill count in `ModData`
- [ ] On milestone kills (100, 500, 1000), broadcast via `sendServerCommand`
- [x] Periodic sync: write kill stats to a JSON file on disk (shared Docker volume)

### Rust backend

- [x] New DB table: `player_stats (season_id, steam_id, zomboid_username, zombie_kills, last_synced_at)`
- [x] Background service: poll kill stats JSON file every 60s, upsert into DB
- [x] `GET /api/leaderboard` — returns top killers for current season (cached in-memory)
- [x] `GET /api/leaderboard?season_id=N` — historical leaderboards (DB fallback)
- [x] On season rotation: check top 3 killers → auto-create 100% discount promo codes
- [ ] RCON `addxp` for kill milestones (optional XP bonus)

### Frontend

- [x] Standalone leaderboard page (`/leaderboard.html`) with season selector
- [x] Link panel added to `index.html`
- [x] Style matches existing terminal aesthetic (gold/silver/bronze for top 3)

---

## 2. Scheduled Horde Events ("Blood Moon Nights")

Every N in-game days, trigger a massive horde. Players get warning via `servermsg`, survivors get XP bonus.

### Rust backend

- [ ] New config fields: `horde_interval_days` (default 7), `horde_size` (default 200)
- [ ] Background service: track in-game day count (via Lua mod time sync or `EveryDays` event)
- [ ] Horde trigger sequence (all via RCON):
  - `servermsg "HORDE NIGHT IN 1 HOUR. FIND SHELTER."` at dusk
  - `createhorde <count>` near each online player (parse `players` command output)
  - `changeoption ZombieCount=4` (spike density during horde)
  - At dawn: `changeoption ZombieCount=<normal>`, `servermsg "Dawn breaks. You survived."`
  - `addxp <survivor> Strength=500` for each player still alive
- [ ] Admin endpoint: `POST /api/admin/horde` — manually trigger a horde event
- [ ] Track horde survival stats in `player_stats` table

### Server-side Lua mod

- [ ] Hook `EveryDays` — send current in-game day to backend via file or `OnClientCommand`
- [ ] Alternative: backend reads game time from PZ save files directly

---

## 3. Supply Drops / Airdrop Events ✅

Random supply drops at known POIs. First player to reach gets the loot. Creates emergent PvE/PvP hotspots.

### Rust backend

- [x] Config: list of POI coordinates with names (Rosewood Gas Station, West Point Mall, etc.)
- [x] Random POI selection
- [x] Background service: random timer (every 6-12 real hours, configurable)
- [x] Drop lifecycle: scheduled → announced (RCON servermsg) → active (Lua spawns loot) → claimed/expired
- [x] JSON file exchange: `drops_pending.json` (backend → Lua), `drops_claimed.json` (Lua → backend)
- [x] Admin endpoint: `POST /api/admin/supply-drops` — trigger drop at specific POI
- [x] Admin endpoint: `GET /api/admin/supply-drops` — list all drops for season
- [x] DB table: `supply_drops` with full lifecycle tracking
- [x] Hard expiry cleanup: stale drops auto-expire, despawn commands sent to Lua
- [x] Duplicate prevention: force-expire old drop before scheduling at same POI

### Server-side Lua mod

- [x] `SupplyDrops.lua`: polls `drops_pending.json`, spawns/despawns loot at grid squares
- [x] Loot tables by tier: standard, military (+ ALICE backpack), medical (+ vitamins, alcohol wipes)
- [x] Claim detection: checks if items taken, finds nearest player, writes `drops_claimed.json`
- [x] ModData persistence for active drops (survives restarts)

### Starter Kit

- [x] `StarterKit.lua`: gives every player a school bag + kitchen knife on first connect
- [x] Tracked via ModData (steam_id) — no duplicates, works for existing + new players

### Frontend

- [x] Supply drops table on index.html (location, status, time, claimed by)
- [x] Auto-refresh every 30s
- [x] Status badges: INCOMING (scheduled/announced), ACTIVE, CLAIMED, EXPIRED

### Public API

- [x] `GET /api/supply-drops` — recent 10 drops for current season (no auth)

---

## 5. Death Tax / Permadeath Stakes

Track deaths per player per season. After N deaths, player must re-register (and optionally pay again).

### Server-side Lua mod

- [ ] Hook `Events.OnPlayerDeath` — write death event to shared JSON file or trigger `sendServerCommand`
- [ ] Include: username, steam_id, in-game timestamp, cause of death (if available)
- [ ] Threat that system, as "how many lives, user can have"

### Rust backend

- [ ] New DB column or table: `death_count` per registration
- [ ] Background service: poll death events, increment counter
- [ ] On death threshold exceeded (e.g., 3 deaths):
  - RCON: `servermsg "<player> has used all their lives."`
  - RCON: `removeuserfromwhitelist <player>` + `removesteamid <steam_id>`
  - Set registration status to `exhausted`
  - Player must re-register on website (new payment or promo code)
- [ ] Config: `max_deaths_per_season` (default 3, 0 = unlimited)
- [ ] `GET /api/register/{id}` — include `deaths_remaining` in response

### Frontend

- [ ] Show deaths remaining on registration status check
- [ ] Registration form: note about limited lives

---

## 6. Discord Webhook Integration

Post key server events to a Discord channel via webhooks.
Use existing project /home/john/dump/git-repos/git-moje/trading-probability-system
to get some knowledge about coding DISCORD WEBHOOK and managing TOML config of it.

### Rust backend

- [ ] New config field: `discord_webhook_url` (optional)
- [ ] Helper: `async fn post_discord(webhook_url, embed)` using `reqwest`
- [ ] Events to post:
  - Registration confirmed: "BINDIN joined Season 01"
  - Player death (from Lua hook): "BINDIN died after 14 days survived"
  - Season rotation: "Season 02 has begun! Register at zomboid.princeofcrypto.com"
  - Horde night: "Horde Night starting in 10 minutes"
  - Supply drop: "Supply drop incoming near Rosewood Gas Station"
  - Kill milestone: "BINDIN reached 1000 zombie kills"
  - Server start/stop
- [ ] Use Discord embed format with colored sidebar (green=join, red=death, orange=event)
- [ ] Rate limiting: batch events, max 1 message per 5 seconds

### Config

```toml
[discord]
some_setting = "xxxx"
[[discord.server]]
webhook_url = "https://discord.com/api/webhooks/..."
```

---

## 8. Hot-Reloadable Server Lua Mod ✅

Ship a server-side Lua mod that the backend can update and hot-reload via RCON without restarting PZ.

### Mod structure (`mod/ZomboidSeasons/`)

```
mod/ZomboidSeasons/
  mod.info
  media/lua/server/ZomboidSeasons/
    Main.lua              -- event hooks, dispatches to modules
    KillTracker.lua       -- OnZombieDead handler, ModData persistence, JSON flush
    SupplyDrops.lua       -- supply drop spawning/despawning/claim detection
    StarterKit.lua        -- school bag + knife on first connect
    Config.lua            -- read config from shared file (backend-writable)
```

### Rust backend

- [x] Deploy mod files to PZ server filesystem via rsync in `scripts/deploy.sh`
- [x] On deploy: RCON `reloadlua "ZomboidSeasons/Main.lua"` (zero-downtime updates)
- [x] Config bridge: write `config.json` to shared data dir (mod reads on load)
- [x] Mod registered via `Mods=ZomboidSeasons` in server ini (`update_server_ini()`)

### Shared data (backend <-> Lua mod)

- [x] Lua writes: `/home/pzuser/Zomboid/ZomboidSeasons/kills.json`
- [x] Lua writes: `drops_claimed.json` (supply drop claims)
- [x] Lua reads: `drops_pending.json` (spawn/despawn commands from backend)
- [ ] Lua writes: `deaths.json`, `gametime.json` (future: tasks #5, #2)
- [x] Backend reads: polls `kills.json` via Docker RO mount at `/data/kills.json`
- [x] Backend reads: polls `drops_claimed.json` via Docker RO mount at `/data/drops_claimed.json`
- [x] Backend writes: `drops_pending.json` to shared data dir
- [x] Backend writes: `/home/pzuser/Zomboid/ZomboidSeasons/config.json`

### Deployment

- [x] Mod files rsynced in `scripts/deploy.sh`
- [x] `update_server_ini()` manages `Mods=` field
- [x] Docker compose mounts `kills.json` and `drops_claimed.json` as read-only
- [x] Deploy script seeds empty `kills.json`, `drops_pending.json`, `drops_claimed.json`
- [x] RCON `reloadlua` after deploy for zero-downtime updates

## 9. Traits via magazines, VHS, CD, etc.

Removal of negative tiers, adding positive tiers. Some tiers have their-counter trait: first unlock removal, then unlock positive (require sequence).
Randomly, using all possible magazines,... generate groups for tiers (solo) and tiers (with counters). Min. 2 to UNLOCK (removal), Min. 4 to UNLOCK (gain).

## 10. Currency - can buy back next season, on that character

## 11. QoL - research what people matters the most - add missing features, if possible

## Extras

- ~~adjust in-game time, we need 6:1 ratio to real-time (instead of 10:1)~~ ✅ DayLength=6
- ~~fix season time to 90 days (and adjust the current)~~ ✅

---

## Implementation Order

1. ~~**Lua mod skeleton (#8)** — foundation for kill/death tracking~~ ✅
2. **Discord webhooks (#6)** — quick win, high visibility, no Lua dependency
3. **Horde nights (#2)** — pure RCON, no Lua needed, high player excitement
4. ~~**Kill leaderboard (#1)** — depends on Lua mod~~ ✅
5. **Death tax (#5)** — depends on Lua mod (add `DeathTracker.lua`)
6. ~~**Supply drops (#3)** — RCON for announcements, Lua for loot placement~~ ✅
