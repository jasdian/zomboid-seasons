-- ZomboidSeasons server-side mod entry point
-- Supports hot-reload via RCON `reloadlua "ZomboidSeasons/Main.lua"`

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons._generation = (ZomboidSeasons._generation or 0) + 1
local gen = ZomboidSeasons._generation

local modules = {
    "ZomboidSeasons/Config",
    "ZomboidSeasons/Utils",
    "ZomboidSeasons/EventTracker",
    "ZomboidSeasons/SessionTracker",
    "ZomboidSeasons/KillTracker",
    "ZomboidSeasons/SupplyDrops",
    "ZomboidSeasons/StarterKit",
    "ZomboidSeasons/AdminCommands",
    "ZomboidSeasons/GameTime",
    "ZomboidSeasons/NoBuildZone",
    "ZomboidSeasons/PvpToggle",
}

if gen > 1 then
    -- Flush pending data before re-loading modules
    if ZomboidSeasons.KillTracker and ZomboidSeasons.KillTracker.flush then
        ZomboidSeasons.KillTracker.flush()
    end
    if ZomboidSeasons.SupplyDrops and ZomboidSeasons.SupplyDrops.flushClaims then
        ZomboidSeasons.SupplyDrops.flushClaims()
    end
    if ZomboidSeasons.EventTracker and ZomboidSeasons.EventTracker.flush then
        ZomboidSeasons.EventTracker.flush()
    end
end

if gen > 1 then
    -- Hot-reload: re-execute each module from disk
    for _, mod in ipairs(modules) do
        local path = mod .. ".lua"
        reloadServerLuaFile(path)
    end
else
    for _, mod in ipairs(modules) do
        require(mod)
    end
end

-- Death tracking hook: emits player_death event with character stats
local function onPlayerDeath(player)
    if not player then return end
    local username = player:getUsername() or "unknown"
    local steamId = ZomboidSeasons.Utils.getSteamId(player)
    local pd = player:getModData()
    local charId = pd.ZS_CharId or ""

    -- Get current world age for survival calculation
    local worldAge = 0
    local gt = getGameTime()
    if gt then worldAge = gt:getWorldAgeHours() end

    -- Get kill count from KillTracker
    local kills = 0
    if ZomboidSeasons.KillTracker and ZomboidSeasons.KillTracker.getKills then
        kills = ZomboidSeasons.KillTracker.getKills(username) or 0
    end

    -- Get PZ's native survival hours for this character
    local hoursSurvived = player:getHoursSurvived() or 0

    local now = getTimestampMs()
    ZomboidSeasons.EventTracker.emit(
        "player_death",
        steamId,
        username,
        { lua_char_id = charId, kills = kills, world_age_hours = worldAge, hours_survived = hoursSurvived },
        "death_" .. steamId .. "_" .. now
    )

    -- Clear character ID so next spawn creates a new character
    pd.ZS_CharId = nil

    -- Flush events immediately on death (important data)
    ZomboidSeasons.EventTracker.flush()

    print("[ZomboidSeasons] Player death: " .. username .. " (kills=" .. kills .. ", worldAge=" .. string.format("%.1f", worldAge) .. ")")
end

local function initAll()
    ZomboidSeasons.Config.load()
    ZomboidSeasons.EventTracker.init()
    ZomboidSeasons.SessionTracker.init()
    ZomboidSeasons.KillTracker.init()
    ZomboidSeasons.SupplyDrops.init()
    ZomboidSeasons.StarterKit.init()
    ZomboidSeasons.AdminCommands.init()
    ZomboidSeasons.GameTime.init()
    ZomboidSeasons.NoBuildZone.init()
    ZomboidSeasons.PvpToggle.init()

    -- Death hook (Remove+Add for hot-reload safety)
    Events.OnPlayerDeath.Remove(onPlayerDeath)
    Events.OnPlayerDeath.Add(onPlayerDeath)

    print("[ZomboidSeasons] Initialized (generation " .. gen .. ")")
end

if gen == 1 then
    Events.OnServerStarted.Add(initAll)
else
    initAll()
end
