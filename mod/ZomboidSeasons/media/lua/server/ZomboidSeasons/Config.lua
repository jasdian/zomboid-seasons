-- ZomboidSeasons configuration module
-- Reads runtime config from a JSON file written by the Rust backend.
-- Falls back to defaults if the config file is absent.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.Config = {}

-- PZ sandboxes getFileReader/getFileWriter under ~/Zomboid/Lua/,
-- so all paths must be relative (resolved as ~/Zomboid/Lua/<path>).
local CONFIG_PATH = "ZomboidSeasons/config.json"

local DEFAULT_CONFIG = {
    data_dir = "ZomboidSeasons",
    sync_interval_minutes = 5,
    steam_id_map = {},
}

--- Minimal JSON string parser for config.json.
--- Handles the subset we emit: strings, numbers, nested objects, arrays.
--- PZ Kahlua2 has no json.decode so we do it by hand.
local function parseJsonValue(s, pos)
    -- skip whitespace
    pos = pos or 1
    while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
    local ch = s:sub(pos, pos)

    if ch == '"' then
        -- string
        local j = pos + 1
        local result = ""
        while j <= #s do
            local c = s:sub(j, j)
            if c == '\\' then
                local nc = s:sub(j + 1, j + 1)
                if nc == '"' then result = result .. '"'
                elseif nc == '\\' then result = result .. '\\'
                elseif nc == 'n' then result = result .. '\n'
                elseif nc == 'r' then result = result .. '\r'
                elseif nc == 't' then result = result .. '\t'
                else result = result .. nc end
                j = j + 2
            elseif c == '"' then
                return result, j + 1
            else
                result = result .. c
                j = j + 1
            end
        end
        return result, j
    elseif ch == '{' then
        -- object
        local obj = {}
        pos = pos + 1
        while true do
            while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
            if s:sub(pos, pos) == '}' then return obj, pos + 1 end
            if s:sub(pos, pos) == ',' then pos = pos + 1 end
            while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
            if s:sub(pos, pos) == '}' then return obj, pos + 1 end
            local key
            key, pos = parseJsonValue(s, pos)
            while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
            if s:sub(pos, pos) == ':' then pos = pos + 1 end
            local val
            val, pos = parseJsonValue(s, pos)
            obj[key] = val
        end
    elseif ch == '[' then
        -- array
        local arr = {}
        pos = pos + 1
        while true do
            while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
            if s:sub(pos, pos) == ']' then return arr, pos + 1 end
            if s:sub(pos, pos) == ',' then pos = pos + 1 end
            while pos <= #s and s:sub(pos, pos):match("%s") do pos = pos + 1 end
            if s:sub(pos, pos) == ']' then return arr, pos + 1 end
            local val
            val, pos = parseJsonValue(s, pos)
            table.insert(arr, val)
        end
    elseif s:sub(pos, pos + 3) == "true" then
        return true, pos + 4
    elseif s:sub(pos, pos + 4) == "false" then
        return false, pos + 5
    elseif s:sub(pos, pos + 3) == "null" then
        return nil, pos + 4
    else
        -- number
        local numStr = s:match("^%-?%d+%.?%d*[eE]?[%+%-]?%d*", pos)
        if numStr then
            return tonumber(numStr), pos + #numStr
        end
        return nil, pos + 1
    end
end

function ZomboidSeasons.Config.load()
    local file = getFileReader(CONFIG_PATH, false)
    if file then
        local content = ""
        local line = file:readLine()
        while line do
            content = content .. line
            line = file:readLine()
        end
        file:close()

        local ok, parsed = pcall(parseJsonValue, content, 1)
        if ok and type(parsed) == "table" then
            -- Merge parsed values over defaults
            for k, v in pairs(parsed) do
                DEFAULT_CONFIG[k] = v
            end
            local mapSize = 0
            if parsed.steam_id_map then
                for _ in pairs(parsed.steam_id_map) do mapSize = mapSize + 1 end
            end
            print("[ZomboidSeasons] Config loaded from " .. CONFIG_PATH .. " (steam_id_map: " .. mapSize .. " entries)")
        else
            print("[ZomboidSeasons] Config parse failed, using defaults")
        end
    else
        print("[ZomboidSeasons] No config file found, using defaults")
    end

    ZomboidSeasons.Config.values = DEFAULT_CONFIG
end

function ZomboidSeasons.Config.get(key)
    return ZomboidSeasons.Config.values[key] or DEFAULT_CONFIG[key]
end

--- Look up the authoritative steam ID for a username.
--- Returns the mapped ID or nil if not found.
function ZomboidSeasons.Config.getSteamId(username)
    local m = ZomboidSeasons.Config.values and ZomboidSeasons.Config.values.steam_id_map
    if m and username then
        return m[username]
    end
    return nil
end
