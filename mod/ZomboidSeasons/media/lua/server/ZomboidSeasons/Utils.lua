-- ZomboidSeasons shared utilities

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.Utils = {}

--- Normalize steam ID to a plain digit string.
--- Kahlua2 tostring() on Java longs may produce scientific notation (e.g.
--- "7.656119798478254E16"). We MUST NOT round-trip through tonumber() because
--- float64 loses precision on 17-digit steam IDs. Instead we try the Java
--- toString() method first (exact), then parse scientific notation as strings.
function ZomboidSeasons.Utils.normalizeSteamId(raw)
    -- Try Java toString() which gives exact representation
    if raw and type(raw) ~= "string" and type(raw) ~= "number" then
        local ok, jstr = pcall(function() return raw:toString() end)
        if ok and jstr then return jstr end
    end
    local s = tostring(raw)
    -- Already a plain integer string — return as-is
    if s:match("^%d+$") then return s end
    -- Handle scientific notation string: shift decimal point
    local mantissa, exp = s:match("^([%d%.]+)[eE](%+?%d+)$")
    if mantissa and exp then
        local e = tonumber(exp)
        local intPart, fracPart = mantissa:match("^(%d+)%.?(%d*)$")
        if intPart then
            local digits = intPart .. fracPart
            local neededLen = #intPart + e
            if #digits < neededLen then
                digits = digits .. string.rep("0", neededLen - #digits)
            elseif #digits > neededLen then
                digits = digits:sub(1, neededLen)
            end
            return digits
        end
    end
    return s
end

--- Escape a string for safe JSON embedding.
function ZomboidSeasons.Utils.jsonEscape(s)
    if not s then return "" end
    s = s:gsub('\\', '\\\\')
    s = s:gsub('"', '\\"')
    s = s:gsub('\n', '\\n')
    s = s:gsub('\r', '\\r')
    s = s:gsub('\t', '\\t')
    return s
end

--- Count keys in a table (PZ Lua has no # for hash tables).
function ZomboidSeasons.Utils.countKeys(t)
    local n = 0
    for _ in pairs(t) do n = n + 1 end
    return n
end

--- Get the correct steam ID for a player object.
--- Prefers config map lookup by username, falls back to normalizeSteamId.
function ZomboidSeasons.Utils.getSteamId(player)
    local username = player:getUsername()
    if username then
        local mapped = ZomboidSeasons.Config.getSteamId(username)
        if mapped then return mapped end
    end
    return ZomboidSeasons.Utils.normalizeSteamId(player:getSteamID())
end

--- Iterate all online players and call fn(player, steamId) for each.
--- steamId is already normalized.
--- Prefers authoritative steam_id_map lookup by username (avoids Kahlua2
--- double-precision loss on 17-digit Java longs), falls back to normalizeSteamId.
function ZomboidSeasons.Utils.forEachOnlinePlayer(fn)
    local players = getOnlinePlayers()
    if not players then return end
    for i = 0, players:size() - 1 do
        local player = players:get(i)
        if player then
            local steamId = ZomboidSeasons.Utils.getSteamId(player)
            if steamId and steamId ~= "" and steamId ~= "0" then
                fn(player, steamId)
            end
        end
    end
end
