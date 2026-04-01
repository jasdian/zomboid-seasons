-- ZomboidSeasons admin inventory management module
-- Polls admin_commands.json for inventory commands, writes results to admin_results.json.
-- Commands: list_inventory, remove_item, remove_all_of_type, clear_inventory

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.AdminCommands = {}

local POLL_INTERVAL_MS = 5 * 1000
local lastPoll = 0

local function getDataDir()
    return ZomboidSeasons.Config.get("data_dir")
end

local function findPlayerByName(username)
    local players = getOnlinePlayers()
    if not players then return nil end
    for i = 0, players:size() - 1 do
        local p = players:get(i)
        if p and p:getUsername() == username then
            return p
        end
    end
    return nil
end

local function writeResult(requestId, success, data)
    local dataDir = getDataDir()
    local path = dataDir .. "/admin_results.json"

    -- Read existing results
    local existing = {}
    local reader = getFileReader(path, false)
    if reader then
        local lines = {}
        local line = reader:readLine()
        while line do
            table.insert(lines, line)
            line = reader:readLine()
        end
        reader:close()
        local content = table.concat(lines, "\n")
        -- Parse existing results array
        for obj in content:gmatch('"results"%s*:%s*%[(.-)%]') do
            for entry in obj:gmatch('{(.-)}') do
                local rid = entry:match('"request_id"%s*:%s*"([^"]*)"')
                if rid then
                    table.insert(existing, entry)
                end
            end
        end
    end

    -- Build new result entry
    local status = success and "ok" or "error"
    local dataJson = data or ""
    local entry = string.format('{"request_id":"%s","status":"%s","data":%s}',
        requestId, status, dataJson)
    table.insert(existing, entry)

    -- Keep only last 20 results
    while #existing > 20 do
        table.remove(existing, 1)
    end

    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write('{"results":[' .. table.concat(existing, ",") .. ']}')
        writer:close()
    end
end

local function escapeJson(s)
    return s:gsub('\\', '\\\\'):gsub('"', '\\"')
end

-- List all items in a player's inventory
local function listInventory(cmd)
    local player = findPlayerByName(cmd.username)
    if not player then
        writeResult(cmd.request_id, false, '"player not found or offline"')
        return
    end

    local inv = player:getInventory()
    if not inv then
        writeResult(cmd.request_id, false, '"no inventory"')
        return
    end

    local items = inv:getItems()
    local counts = {}
    local names = {}
    for i = 0, items:size() - 1 do
        local item = items:get(i)
        local ft = item:getFullType()
        counts[ft] = (counts[ft] or 0) + 1
        if not names[ft] then
            names[ft] = item:getDisplayName() or ft
        end
    end

    local parts = {}
    for ft, count in pairs(counts) do
        table.insert(parts, string.format('{"type":"%s","name":"%s","count":%d}',
            escapeJson(ft), escapeJson(names[ft]), count))
    end

    writeResult(cmd.request_id, true, '[' .. table.concat(parts, ",") .. ']')
    print("[ZomboidSeasons] AdminCommands: Listed " .. items:size() .. " items for " .. cmd.username)
end

-- Remove N items of a specific type
local function removeItem(cmd)
    local player = findPlayerByName(cmd.username)
    if not player then
        writeResult(cmd.request_id, false, '"player not found or offline"')
        return
    end

    local inv = player:getInventory()
    if not inv then
        writeResult(cmd.request_id, false, '"no inventory"')
        return
    end

    local count = cmd.count or 1
    local removed = 0
    local items = inv:getItems()

    -- Iterate backwards to safely remove
    for i = items:size() - 1, 0, -1 do
        if removed >= count then break end
        local item = items:get(i)
        if item:getFullType() == cmd.item_type then
            inv:Remove(item)
            removed = removed + 1
        end
    end

    writeResult(cmd.request_id, true, string.format('{"removed":%d}', removed))
    print("[ZomboidSeasons] AdminCommands: Removed " .. removed .. "x " .. cmd.item_type .. " from " .. cmd.username)
end

-- Remove ALL items of a specific type
local function removeAllOfType(cmd)
    local player = findPlayerByName(cmd.username)
    if not player then
        writeResult(cmd.request_id, false, '"player not found or offline"')
        return
    end

    local inv = player:getInventory()
    if not inv then
        writeResult(cmd.request_id, false, '"no inventory"')
        return
    end

    local removed = 0
    local items = inv:getItems()
    for i = items:size() - 1, 0, -1 do
        local item = items:get(i)
        if item:getFullType() == cmd.item_type then
            inv:Remove(item)
            removed = removed + 1
        end
    end

    writeResult(cmd.request_id, true, string.format('{"removed":%d}', removed))
    print("[ZomboidSeasons] AdminCommands: Removed all " .. removed .. "x " .. cmd.item_type .. " from " .. cmd.username)
end

-- Clear entire inventory
local function clearInventory(cmd)
    local player = findPlayerByName(cmd.username)
    if not player then
        writeResult(cmd.request_id, false, '"player not found or offline"')
        return
    end

    local inv = player:getInventory()
    if not inv then
        writeResult(cmd.request_id, false, '"no inventory"')
        return
    end

    local count = inv:getItems():size()
    inv:removeAllItems()

    writeResult(cmd.request_id, true, string.format('{"removed":%d}', count))
    print("[ZomboidSeasons] AdminCommands: Cleared " .. count .. " items from " .. cmd.username)
end

function ZomboidSeasons.AdminCommands.poll()
    local now = getTimestampMs()
    if now - lastPoll < POLL_INTERVAL_MS then return end
    lastPoll = now

    local dataDir = getDataDir()
    local path = dataDir .. "/admin_commands.json"

    local reader = getFileReader(path, false)
    if not reader then return end

    local lines = {}
    local line = reader:readLine()
    while line do
        table.insert(lines, line)
        line = reader:readLine()
    end
    reader:close()

    local content = table.concat(lines, "\n")
    if content == "" or content == '{"commands":[]}' then return end

    -- Parse commands
    local commands = {}
    for block in content:gmatch('"commands"%s*:%s*%[(.-)%]') do
        for obj in block:gmatch('{(.-)}') do
            local cmd = {}
            cmd.action = obj:match('"action"%s*:%s*"([^"]*)"')
            cmd.request_id = obj:match('"request_id"%s*:%s*"([^"]*)"')
            cmd.username = obj:match('"username"%s*:%s*"([^"]*)"')
            cmd.item_type = obj:match('"item_type"%s*:%s*"([^"]*)"')
            local countStr = obj:match('"count"%s*:%s*(%d+)')
            cmd.count = countStr and tonumber(countStr) or nil
            if cmd.action and cmd.request_id then
                table.insert(commands, cmd)
            end
        end
    end

    -- Execute commands
    for _, cmd in ipairs(commands) do
        if cmd.action == "list_inventory" then
            listInventory(cmd)
        elseif cmd.action == "remove_item" then
            removeItem(cmd)
        elseif cmd.action == "remove_all_of_type" then
            removeAllOfType(cmd)
        elseif cmd.action == "clear_inventory" then
            clearInventory(cmd)
        else
            writeResult(cmd.request_id, false, '"unknown action: ' .. (cmd.action or '') .. '"')
        end
    end

    -- Clear processed commands
    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write('{"commands":[]}')
        writer:close()
    end
end

function ZomboidSeasons.AdminCommands.init()
    Events.EveryOneMinute.Remove(ZomboidSeasons.AdminCommands.poll)
    Events.EveryOneMinute.Add(ZomboidSeasons.AdminCommands.poll)
    print("[ZomboidSeasons] AdminCommands initialized")
end
