-- ZomboidSeasons kill tracking module
-- Hooks OnZombieDead to count per-player kills, persists via ModData,
-- and periodically flushes a JSON file for the Rust backend to poll.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.KillTracker = {}

local killCounts = {}  -- username -> { steam_id = str, kills = int }
local dirty = false
local lastFlush = 0
local FLUSH_INTERVAL_MS = 5 * 60 * 1000  -- 5 minutes

-- Count keys in a table (PZ Lua has no # for hash tables)
local function countKeys(t)
    local n = 0
    for _ in pairs(t) do n = n + 1 end
    return n
end

local normalizeSteamId = function(raw)
    return ZomboidSeasons.Utils.normalizeSteamId(raw)
end

function ZomboidSeasons.KillTracker.init()
    -- Restore persisted counts from ModData (survives server restart within a season)
    local modData = ModData.getOrCreate("ZomboidSeasons_Kills")
    if modData.players then
        for key, data in pairs(modData.players) do
            local username = data.username or key
            -- Skip ghost entries where key is a steam_id (17+ digit string)
            -- and no explicit username was stored (pre-migration artifact)
            if not data.username and string.len(key) >= 15 and string.match(key, "^%d+$") then
                print("[ZomboidSeasons] KillTracker: Skipping steam_id-keyed entry: " .. key)
            else
                local steamId = data.steam_id or normalizeSteamId(key)
                if killCounts[username] then
                    -- Merge: keep higher kill count (migration from steam_id keying)
                    if data.kills > killCounts[username].kills then
                        killCounts[username].kills = data.kills
                    end
                else
                    killCounts[username] = { steam_id = steamId, kills = data.kills }
                end
            end
        end
        dirty = true  -- re-flush to fix persisted keys
        print("[ZomboidSeasons] Loaded " .. countKeys(killCounts) .. " player kill records from ModData")
    end

    -- Remove-before-Add guards for hot-reload safety (prevents duplicate hooks)
    Events.OnZombieDead.Remove(ZomboidSeasons.KillTracker.onZombieDead)
    Events.EveryOneMinute.Remove(ZomboidSeasons.KillTracker.onTick)
    Events.OnZombieDead.Add(ZomboidSeasons.KillTracker.onZombieDead)
    Events.EveryOneMinute.Add(ZomboidSeasons.KillTracker.onTick)

    print("[ZomboidSeasons] KillTracker initialized")
end

function ZomboidSeasons.KillTracker.onZombieDead(zombie)
    local killer = zombie:getAttackedBy()
    if not killer then return end
    if not instanceof(killer, "IsoPlayer") then return end

    local username = killer:getUsername()
    if not username or username == "" then return end

    local steamId = ZomboidSeasons.Utils.getSteamId(killer)

    if not killCounts[username] then
        killCounts[username] = { steam_id = steamId, kills = 0 }
    end

    killCounts[username].kills = killCounts[username].kills + 1
    killCounts[username].steam_id = steamId  -- keep steam_id current
    dirty = true

    -- Emit event for the new reward system
    local charId = killer:getModData().ZS_CharId or ""
    local killNum = killCounts[username].kills
    ZomboidSeasons.EventTracker.emit(
        "zombie_kill",
        steamId,
        username,
        { lua_char_id = charId },
        "zk_" .. steamId .. "_" .. getTimestampMs() .. "_" .. killNum
    )
end

function ZomboidSeasons.KillTracker.onTick()
    if not dirty then return end

    local now = getTimestampMs()
    if now - lastFlush < FLUSH_INTERVAL_MS then return end

    ZomboidSeasons.KillTracker.flush()
    lastFlush = now
end

--- Get the current kill count for a username.
function ZomboidSeasons.KillTracker.getKills(username)
    local entry = killCounts[username]
    return entry and entry.kills or 0
end

function ZomboidSeasons.KillTracker.flush()
    if not dirty then return end

    -- Persist to ModData (survives restart, stays with save)
    local modData = ModData.getOrCreate("ZomboidSeasons_Kills")
    modData.players = {}
    for username, data in pairs(killCounts) do
        modData.players[username] = { steam_id = data.steam_id, kills = data.kills }
    end
    ModData.transmit("ZomboidSeasons_Kills")

    -- Write JSON for the Rust backend to poll
    local dataDir = ZomboidSeasons.Config.get("data_dir")
    local path = dataDir .. "/kills.json"

    -- Build JSON manually (PZ Lua / Kahlua2 has no json.encode)
    local esc = ZomboidSeasons.Utils.jsonEscape
    local parts = {}
    for username, data in pairs(killCounts) do
        table.insert(parts, string.format(
            '{"steam_id":"%s","username":"%s","kills":%d}',
            esc(data.steam_id), esc(username), data.kills
        ))
    end
    local json = '{"players":[' .. table.concat(parts, ",") .. ']}'

    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write(json)
        writer:close()
        print("[ZomboidSeasons] Flushed " .. countKeys(killCounts) .. " player stats to " .. path)
    else
        print("[ZomboidSeasons] ERROR: Could not write to " .. path)
    end

    dirty = false
end
