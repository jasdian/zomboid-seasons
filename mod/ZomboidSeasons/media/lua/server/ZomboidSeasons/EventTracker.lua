-- ZomboidSeasons event tracking module
-- Buffers game events (kills, deaths, logins, character creation) and
-- periodically flushes them to events.json for the Rust backend to ingest.
-- Uses idempotency keys so events can be safely re-processed.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.EventTracker = {}

local eventBuffer = {}
local lastFlush = 0
local FLUSH_INTERVAL_MS = 60 * 1000  -- 1 minute

local function jsonEscape(s)
    return ZomboidSeasons.Utils.jsonEscape(s)
end

--- Add an event to the buffer.
--- @param eventType string One of: character_created, zombie_kill, player_death, login, logout
--- @param steamId string Normalized 17-digit Steam ID
--- @param username string PZ username
--- @param payload table Event-specific data (will be serialized to JSON)
--- @param idempotencyKey string Unique key to prevent duplicate processing
function ZomboidSeasons.EventTracker.emit(eventType, steamId, username, payload, idempotencyKey)
    table.insert(eventBuffer, {
        type = eventType,
        steam_id = steamId,
        username = username,
        payload = payload or {},
        key = idempotencyKey,
        ts = getTimestampMs(),
    })
end

--- Serialize a flat payload table to a JSON object string.
--- Supports string, number, and boolean values only (no nesting).
local function payloadToJson(payload)
    if not payload then return "{}" end
    local parts = {}
    for k, v in pairs(payload) do
        local valStr
        if type(v) == "string" then
            valStr = '"' .. jsonEscape(v) .. '"'
        elseif type(v) == "number" then
            -- Use %g for integers, %.2f for floats
            if v == math.floor(v) then
                valStr = string.format("%d", v)
            else
                valStr = string.format("%.2f", v)
            end
        elseif type(v) == "boolean" then
            valStr = v and "true" or "false"
        else
            valStr = '"' .. jsonEscape(tostring(v)) .. '"'
        end
        table.insert(parts, '"' .. jsonEscape(k) .. '":' .. valStr)
    end
    return "{" .. table.concat(parts, ",") .. "}"
end

function ZomboidSeasons.EventTracker.flush()
    if #eventBuffer == 0 then return end

    local dataDir = ZomboidSeasons.Config.get("data_dir")
    local path = dataDir .. "/events.json"

    -- Build JSON array of events
    local parts = {}
    for _, evt in ipairs(eventBuffer) do
        local line = string.format(
            '{"type":"%s","steam_id":"%s","username":"%s","payload":%s,"ts":%d,"key":"%s"}',
            jsonEscape(evt.type),
            jsonEscape(evt.steam_id),
            jsonEscape(evt.username),
            payloadToJson(evt.payload),
            evt.ts,
            jsonEscape(evt.key)
        )
        table.insert(parts, line)
    end
    local json = '{"events":[' .. table.concat(parts, ",") .. ']}'

    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write(json)
        writer:close()
        print("[ZomboidSeasons] EventTracker: flushed " .. #eventBuffer .. " events to " .. path)
    else
        print("[ZomboidSeasons] EventTracker: ERROR could not write to " .. path)
        return  -- keep buffer for retry
    end

    eventBuffer = {}
end

function ZomboidSeasons.EventTracker.onTick()
    if #eventBuffer == 0 then return end

    local now = getTimestampMs()
    if now - lastFlush < FLUSH_INTERVAL_MS then return end

    ZomboidSeasons.EventTracker.flush()
    lastFlush = now
end

function ZomboidSeasons.EventTracker.init()
    Events.EveryOneMinute.Remove(ZomboidSeasons.EventTracker.onTick)
    Events.EveryOneMinute.Add(ZomboidSeasons.EventTracker.onTick)
    print("[ZomboidSeasons] EventTracker initialized")
end
