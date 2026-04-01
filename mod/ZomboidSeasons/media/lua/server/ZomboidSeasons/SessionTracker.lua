-- ZomboidSeasons session tracking module
-- Detects logins/logouts and syncs hours_survived for all online players every tick.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.SessionTracker = {}

-- { steamId -> { username, login_ts (ms), last_seen_ts (ms), char_id, hours_survived } }
local activeSessions = {}

local normalizeSteamId = function(raw)
    return ZomboidSeasons.Utils.normalizeSteamId(raw)
end

function ZomboidSeasons.SessionTracker.init()
    local modData = ModData.getOrCreate("ZomboidSeasons_Sessions")
    if modData.sessions then
        for steamId, data in pairs(modData.sessions) do
            activeSessions[steamId] = {
                username = data.username,
                login_ts = data.login_ts,
                last_seen_ts = data.last_seen_ts or data.login_ts,
                char_id = data.char_id or "",
                hours_survived = data.hours_survived or 0,
            }
        end
        print("[ZomboidSeasons] SessionTracker: restored " .. ZomboidSeasons.Utils.countKeys(activeSessions) .. " sessions from ModData")
    end

    Events.EveryOneMinute.Remove(ZomboidSeasons.SessionTracker.tick)
    Events.EveryOneMinute.Add(ZomboidSeasons.SessionTracker.tick)

    Events.OnTick.Remove(ZomboidSeasons.SessionTracker.fastTick)
    Events.OnTick.Add(ZomboidSeasons.SessionTracker.fastTick)

    -- OnPlayerDisconnect may not exist in Build 42; fastTick handles disconnect detection
    if Events.OnPlayerDisconnect then
        Events.OnPlayerDisconnect.Remove(ZomboidSeasons.SessionTracker.onPlayerDisconnect)
        Events.OnPlayerDisconnect.Add(ZomboidSeasons.SessionTracker.onPlayerDisconnect)
    end

    print("[ZomboidSeasons] SessionTracker initialized")
end

local function persistModData()
    local modData = ModData.getOrCreate("ZomboidSeasons_Sessions")
    modData.sessions = {}
    for steamId, session in pairs(activeSessions) do
        modData.sessions[steamId] = {
            username = session.username,
            login_ts = session.login_ts,
            last_seen_ts = session.last_seen_ts,
            char_id = session.char_id,
            hours_survived = session.hours_survived,
        }
    end
end

-- Primary disconnect handler: fires synchronously before PauseEmpty takes effect
function ZomboidSeasons.SessionTracker.onPlayerDisconnect(player)
    local steamId = ZomboidSeasons.Utils.getSteamId(player)
    local session = activeSessions[steamId]
    if not session then return end

    local now = getTimestampMs()
    local minutesOnline = math.floor((now - session.login_ts) / 60000)
    if minutesOnline < 1 then minutesOnline = 1 end

    local hs = player:getHoursSurvived() or session.hours_survived or 0

    ZomboidSeasons.EventTracker.emit(
        "logout",
        steamId,
        session.username,
        { minutes_online = minutesOnline, char_id = session.char_id or "", hours_survived = hs },
        "logout_" .. steamId .. "_" .. now
    )
    ZomboidSeasons.EventTracker.flush()

    print("[ZomboidSeasons] SessionTracker: logout " .. session.username .. " (" .. steamId .. ") after " .. minutesOnline .. " min, hs=" .. string.format("%.1f", hs))
    activeSessions[steamId] = nil
    persistModData()
end

-- Safety net: detect disconnects missed by OnPlayerDisconnect (e.g. server crash recovery)
function ZomboidSeasons.SessionTracker.fastTick()
    local hasAny = false
    for _ in pairs(activeSessions) do hasAny = true; break end
    if not hasAny then return end

    local currentOnline = {}
    ZomboidSeasons.Utils.forEachOnlinePlayer(function(player, steamId)
        currentOnline[steamId] = player
    end)

    for steamId, session in pairs(activeSessions) do
        if not currentOnline[steamId] then
            local now = getTimestampMs()
            local minutesOnline = math.floor((now - session.login_ts) / 60000)
            if minutesOnline < 1 then minutesOnline = 1 end

            ZomboidSeasons.EventTracker.emit(
                "logout",
                steamId,
                session.username,
                { minutes_online = minutesOnline, char_id = session.char_id or "", hours_survived = session.hours_survived or 0 },
                "logout_" .. steamId .. "_" .. now
            )
            ZomboidSeasons.EventTracker.flush()

            print("[ZomboidSeasons] SessionTracker: logout " .. session.username .. " (" .. steamId .. ") after " .. minutesOnline .. " min, hs=" .. string.format("%.1f", session.hours_survived or 0))
            activeSessions[steamId] = nil
        end
    end
end

-- Every minute: detect logins, sync hours_survived for all online players
function ZomboidSeasons.SessionTracker.tick()
    local now = getTimestampMs()

    ZomboidSeasons.Utils.forEachOnlinePlayer(function(player, steamId)
        local username = player:getUsername() or "unknown"
        local pd = player:getModData()
        local charId = (pd and pd.ZS_CharId) or ""
        local hs = player:getHoursSurvived() or 0

        if not activeSessions[steamId] then
            activeSessions[steamId] = {
                username = username,
                login_ts = now,
                last_seen_ts = now,
                char_id = charId,
                hours_survived = hs,
            }
            ZomboidSeasons.EventTracker.emit(
                "login", steamId, username, {},
                "login_" .. steamId .. "_" .. now
            )
            print("[ZomboidSeasons] SessionTracker: login " .. username .. " (" .. steamId .. ")")
        else
            activeSessions[steamId].last_seen_ts = now
            activeSessions[steamId].username = username
            -- Detect character change (death + new character): reset peak tracking
            if charId ~= "" and charId ~= activeSessions[steamId].char_id then
                activeSessions[steamId].char_id = charId
                activeSessions[steamId].hours_survived = hs
            elseif charId ~= "" then
                if hs > (activeSessions[steamId].hours_survived or 0) then
                    activeSessions[steamId].hours_survived = hs
                end
            end
        end

        -- Sync hours_survived to backend for every online player every tick
        if hs > 0 then
            ZomboidSeasons.EventTracker.emit(
                "sync_survival", steamId, username,
                { hours_survived = hs, char_id = charId },
                "sync_" .. steamId .. "_" .. now
            )
        end
    end)

    persistModData()
end

function ZomboidSeasons.SessionTracker.getSession(steamId)
    return activeSessions[steamId]
end
