-- ZomboidSeasons supply drop module
-- Reads drops_pending.json from the backend, spawns/despawns loot containers,
-- detects claims, and writes drops_claimed.json for the backend to poll.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.SupplyDrops = {}

local activeDrops = {}  -- drop_id -> { x, y, z, poi_name, items = { worldItem, ... } }
local claims = {}       -- pending claims to flush
local POLL_INTERVAL_MS = 30 * 1000
local lastPoll = 0

local LOOT_TABLES = {
    military = {
        subgroups = {
            [1] = { "Base.Helmet_Army", "Base.BulletproofVest" },
            [2] = { "Base.AssaultRifle", "Base.AssaultRifle2Magazine" },
            [3] = { "Base.223Box", "Base.223Box", "Base.223Box" },
            [4] = { "Base.ShotgunShellsBox", "Base.ShotgunShellsBox" },
            [5] = { "Base.KnifeMilitary", "Base.HolsterSimple" },
            [6] = { "Base.FirstAidKit", "Base.Bag_ALICEpack_Army" },
            [7] = { "Base.AssaultRifle", "Base.223Box", "Base.223Box", "Base.223Box", "Base.BulletproofVest", "Base.Helmet_Army" },
            [8] = { "Base.Bag_ALICEpack_Army", "Base.KnifeMilitary", "Base.AssaultRifle", "Base.AssaultRifle2Magazine", "Base.223Box", "Base.223Box" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    standard = {
        subgroups = {
            [1] = { "Base.Axe", "Base.Hammer" },
            [2] = { "Base.Shotgun", "Base.ShotgunShellsBox" },
            [3] = { "Base.FirstAidKit", "Base.CannedBeans" },
            [4] = { "Base.NailsBox", "Base.WaterBottleFull" },
            [5] = { "Base.RifleScope", "Base.ShotgunShellsBox", "Base.ShotgunShellsBox" },
            [6] = { "Base.Axe", "Base.Hammer", "Base.NailsBox" },
            [7] = { "Base.Shotgun", "Base.ShotgunShellsBox", "Base.ShotgunShellsBox", "Base.ShotgunShellsBox", "Base.RifleScope" },
            [8] = { "Base.Axe", "Base.Shotgun", "Base.FirstAidKit", "Base.ShotgunShellsBox", "Base.ShotgunShellsBox" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    medical = {
        subgroups = {
            [1] = { "Base.FirstAidKit", "Base.Bandage" },
            [2] = { "Base.Antibiotics", "Base.PainKillers" },
            [3] = { "Base.Disinfectant", "Base.AlcoholWipes" },
            [4] = { "Base.Bandage", "Base.SutureNeedle" },
            [5] = { "Base.PillsVitamins", "Base.PainKillers" },
            [6] = { "Base.Antibiotics", "Base.Disinfectant" },
            [7] = { "Base.FirstAidKit", "Base.Antibiotics", "Base.SutureNeedle", "Base.Bandage", "Base.Bandage" },
            [8] = { "Base.FirstAidKit", "Base.Antibiotics", "Base.PainKillers", "Base.Disinfectant", "Base.SutureNeedle", "Base.AlcoholWipes" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    survival = {
        subgroups = {
            [1] = { "Base.CannedBeans", "Base.CannedBeans", "Base.TinOpener" },
            [2] = { "Base.WaterBottleFull", "Base.CanteenMilitaryFull" },
            [3] = { "Base.Lighter", "Base.Rope" },
            [4] = { "Base.Axe", "Base.FirstAidKit" },
            [5] = { "Base.CompassDirectional", "Base.TentGreen_Packed" },
            [6] = { "Base.Bag_BigHikingBag", "Base.Rope" },
            [7] = { "Base.TentGreen_Packed", "Base.Bag_BigHikingBag", "Base.CanteenMilitaryFull", "Base.CompassDirectional" },
            [8] = { "Base.Bag_BigHikingBag", "Base.TentGreen_Packed", "Base.Axe", "Base.FirstAidKit", "Base.CannedBeans", "Base.CannedBeans" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    builder = {
        subgroups = {
            [1] = { "Base.Hammer", "Base.NailsBox" },
            [2] = { "Base.Saw", "Base.Screwdriver" },
            [3] = { "Base.Plank", "Base.Plank", "Base.NailsBox" },
            [4] = { "Base.DuctTape", "Base.Plank", "Base.Plank" },
            [5] = { "Base.NailsBox", "Base.NailsBox", "Base.Screwdriver" },
            [6] = { "Base.Saw", "Base.Hammer", "Base.DuctTape" },
            [7] = { "Base.Generator", "Base.Hammer", "Base.NailsBox", "Base.NailsBox" },
            [8] = { "Base.Generator", "Base.Saw", "Base.Plank", "Base.Plank", "Base.Plank", "Base.Plank", "Base.NailsBox", "Base.NailsBox" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    tactical = {
        subgroups = {
            [1] = { "Base.Pistol", "Base.Bullets9mmBox" },
            [2] = { "Base.Revolver", "Base.ShotgunShellsBox" },
            [3] = { "Base.WalkieTalkie4", "Base.WalkieTalkie4" },
            [4] = { "Base.Lighter", "Base.Rope" },
            [5] = { "Base.KnifeMilitary", "Base.HolsterShoulder" },
            [6] = { "Base.Bullets9mmBox", "Base.ShotgunShellsBox" },
            [7] = { "Base.Pistol", "Base.Bullets9mmBox", "Base.Bullets9mmBox", "Base.KnifeMilitary", "Base.HolsterShoulder" },
            [8] = { "Base.Pistol", "Base.Revolver", "Base.Bullets9mmBox", "Base.Bullets9mmBox", "Base.ShotgunShellsBox", "Base.KnifeMilitary" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
    mixed = {
        subgroups = {
            [1] = { "Base.Axe", "Base.Hammer" },
            [2] = { "Base.Shotgun", "Base.ShotgunShellsBox" },
            [3] = { "Base.FirstAidKit", "Base.Bandage" },
            [4] = { "Base.CannedBeans", "Base.WaterBottleFull" },
            [5] = { "Base.NailsBox", "Base.Lighter" },
            [6] = { "Base.Bag_NormalHikingBag", "Base.Lighter" },
            [7] = { "Base.Shotgun", "Base.ShotgunShellsBox", "Base.Axe", "Base.FirstAidKit", "Base.Bag_NormalHikingBag" },
            [8] = { "Base.Shotgun", "Base.ShotgunShellsBox", "Base.Axe", "Base.FirstAidKit", "Base.CannedBeans", "Base.WaterBottleFull", "Base.Bag_NormalHikingBag" },
        },
        slots = {
            { chance = 100, candidates = {1, 2, 3} },
            { chance = 50,  candidates = {2, 3, 4} },
            { chance = 20,  candidates = {5, 6} },
            { chance = 1,   candidates = {7, 8} },
        },
    },
}

-- Resolve loot for a tier by rolling probability slots and picking sub-groups
local function resolveLoot(tier)
    local group = LOOT_TABLES[tier] or LOOT_TABLES["standard"]
    local items = {}
    for _, slot in ipairs(group.slots) do
        local roll = ZombRand(100)  -- 0-99
        if roll < slot.chance then
            local pick = slot.candidates[ZombRand(#slot.candidates) + 1]
            local sg = group.subgroups[pick]
            if sg then
                for _, item in ipairs(sg) do
                    table.insert(items, item)
                end
            end
        end
    end
    return items
end

-- Count keys in a table
local function countKeys(t)
    local n = 0
    for _ in pairs(t) do n = n + 1 end
    return n
end

function ZomboidSeasons.SupplyDrops.init()
    -- Restore activeDrops from ModData
    local modData = ModData.getOrCreate("ZomboidSeasons_Drops")
    if modData.activeDrops then
        for dropId, data in pairs(modData.activeDrops) do
            activeDrops[dropId] = {
                x = data.x, y = data.y, z = data.z,
                poi_name = data.poi_name, items = {}
            }
        end
        print("[ZomboidSeasons] Restored " .. countKeys(activeDrops) .. " active supply drops from ModData")
    end

    -- Remove-before-Add guard for hot-reload safety (prevents duplicate hooks)
    Events.EveryOneMinute.Remove(ZomboidSeasons.SupplyDrops.onTick)
    Events.EveryOneMinute.Add(ZomboidSeasons.SupplyDrops.onTick)
    print("[ZomboidSeasons] SupplyDrops initialized")
end

function ZomboidSeasons.SupplyDrops.onTick()
    local now = getTimestampMs()
    if now - lastPoll < POLL_INTERVAL_MS then return end
    lastPoll = now

    ZomboidSeasons.SupplyDrops.pollPending()
    ZomboidSeasons.SupplyDrops.checkClaims()
    ZomboidSeasons.SupplyDrops.flushClaims()
end

function ZomboidSeasons.SupplyDrops.pollPending()
    -- PZ sandboxes getFileReader/getFileWriter under ~/Zomboid/Lua/,
    -- so use the relative data_dir (e.g. "ZomboidSeasons").
    local dataDir = ZomboidSeasons.Config.get("data_dir")
    local path = dataDir .. "/drops_pending.json"

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

    -- Very basic JSON array parsing for commands
    -- Each command is between { } within the "commands" array
    local commands = {}
    for block in content:gmatch('"commands"%s*:%s*%[(.-)%]') do
        for obj in block:gmatch('{(.-)}') do
            local cmd = {}
            cmd.action = obj:match('"action"%s*:%s*"([^"]*)"')
            cmd.drop_id = obj:match('"drop_id"%s*:%s*"([^"]*)"')
            cmd.poi_name = obj:match('"poi_name"%s*:%s*"([^"]*)"')
            local xStr = obj:match('"x"%s*:%s*(%d+)')
            local yStr = obj:match('"y"%s*:%s*(%d+)')
            local zStr = obj:match('"z"%s*:%s*(%d+)')
            cmd.x = xStr and tonumber(xStr) or nil
            cmd.y = yStr and tonumber(yStr) or nil
            cmd.z = zStr and tonumber(zStr) or nil
            cmd.loot_tier = obj:match('"loot_tier"%s*:%s*"([^"]*)"')
            if cmd.action then
                table.insert(commands, cmd)
            end
        end
    end

    for _, cmd in ipairs(commands) do
        if cmd.action == "spawn" then
            ZomboidSeasons.SupplyDrops.spawnDrop(cmd)
        elseif cmd.action == "despawn" then
            ZomboidSeasons.SupplyDrops.despawnDrop(cmd.drop_id)
        end
    end

    -- Clear processed commands by writing empty file
    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write('{"commands":[]}')
        writer:close()
    end
end

function ZomboidSeasons.SupplyDrops.spawnDrop(cmd)
    if not cmd.x or not cmd.y then
        print("[ZomboidSeasons] ERROR: spawn command missing coordinates for " .. tostring(cmd.drop_id))
        return
    end

    local z = cmd.z or 0
    local cell = getCell()
    if not cell then
        print("[ZomboidSeasons] ERROR: cell not loaded for spawn")
        return
    end

    local sq = cell:getGridSquare(cmd.x, cmd.y, z)
    if not sq then
        print("[ZomboidSeasons] ERROR: grid square not found at " .. cmd.x .. "," .. cmd.y .. "," .. z)
        return
    end

    local tier = cmd.loot_tier or "standard"
    local lootItems = resolveLoot(tier)

    local spawnedItems = {}
    for _, itemType in ipairs(lootItems) do
        local item = sq:AddWorldInventoryItem(itemType, 0.0, 0.0, 0.0)
        if item then
            table.insert(spawnedItems, item)
        end
    end

    activeDrops[cmd.drop_id] = {
        x = cmd.x, y = cmd.y, z = z,
        poi_name = cmd.poi_name or "Unknown",
        items = spawnedItems
    }

    -- Persist to ModData
    ZomboidSeasons.SupplyDrops.saveModData()

    -- Create a loud sound at drop location to attract zombies and alert nearby players
    addSound(nil, cmd.x, cmd.y, z, 200, 100)

    print("[ZomboidSeasons] Spawned supply drop " .. cmd.drop_id .. " at " .. cmd.x .. "," .. cmd.y .. "," .. z .. " (" .. tier .. ")")
end

function ZomboidSeasons.SupplyDrops.despawnDrop(dropId)
    local drop = activeDrops[dropId]
    if not drop then return end

    local cell = getCell()
    if cell then
        local sq = cell:getGridSquare(drop.x, drop.y, drop.z)
        if sq and drop.items then
            for _, item in ipairs(drop.items) do
                if item then
                    sq:removeWorldObject(item)
                end
            end
        end
    end

    activeDrops[dropId] = nil
    ZomboidSeasons.SupplyDrops.saveModData()

    print("[ZomboidSeasons] Despawned supply drop " .. dropId)
end

function ZomboidSeasons.SupplyDrops.checkClaims()
    for dropId, drop in pairs(activeDrops) do
        local cell = getCell()
        if not cell then break end

        local sq = cell:getGridSquare(drop.x, drop.y, drop.z)
        if not sq then break end

        -- Check if loot items are gone (taken by player)
        local itemsRemaining = 0
        if drop.items then
            for _, item in ipairs(drop.items) do
                if item and sq:getWorldObjects():contains(item) then
                    itemsRemaining = itemsRemaining + 1
                end
            end
        end

        -- If more than half the items are gone, consider it claimed
        local totalItems = drop.items and #drop.items or 0
        if totalItems > 0 and itemsRemaining < (totalItems / 2) then
            -- Find nearest player
            local nearestPlayer = nil
            local nearestDist = 15 * 15  -- 15 tile radius squared

            local players = getOnlinePlayers()
            if players then
                for i = 0, players:size() - 1 do
                    local p = players:get(i)
                    if p then
                        local dx = p:getX() - drop.x
                        local dy = p:getY() - drop.y
                        local distSq = dx * dx + dy * dy
                        if distSq < nearestDist then
                            nearestDist = distSq
                            nearestPlayer = p
                        end
                    end
                end
            end

            if nearestPlayer then
                local steamId = ZomboidSeasons.Utils.getSteamId(nearestPlayer)
                local username = nearestPlayer:getUsername()
                table.insert(claims, {
                    drop_id = dropId,
                    steam_id = steamId,
                    username = username
                })
                print("[ZomboidSeasons] Drop " .. dropId .. " claimed by " .. username)
            end

            -- Remove from active regardless
            activeDrops[dropId] = nil
            ZomboidSeasons.SupplyDrops.saveModData()
        end
    end
end

function ZomboidSeasons.SupplyDrops.flushClaims()
    if #claims == 0 then return end

    local dataDir = ZomboidSeasons.Config.get("data_dir")
    local path = dataDir .. "/drops_claimed.json"

    -- Read existing claims
    local existingClaims = {}
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
        -- Parse existing claims array
        for obj in content:gmatch('"claims"%s*:%s*%[(.-)%]') do
            for entry in obj:gmatch('{(.-)}') do
                local dropId = entry:match('"drop_id"%s*:%s*"([^"]*)"')
                local steamId = entry:match('"steam_id"%s*:%s*"([^"]*)"')
                local username = entry:match('"username"%s*:%s*"([^"]*)"')
                if dropId then
                    table.insert(existingClaims, {
                        drop_id = dropId,
                        steam_id = steamId or "",
                        username = username or ""
                    })
                end
            end
        end
    end

    -- Merge new claims
    for _, c in ipairs(claims) do
        table.insert(existingClaims, c)
    end

    -- Build JSON manually
    local parts = {}
    for _, c in ipairs(existingClaims) do
        table.insert(parts, string.format(
            '{"drop_id":"%s","steam_id":"%s","username":"%s"}',
            c.drop_id, c.steam_id, c.username
        ))
    end
    local json = '{"claims":[' .. table.concat(parts, ",") .. ']}'

    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write(json)
        writer:close()
        print("[ZomboidSeasons] Flushed " .. #claims .. " supply drop claims")
    end

    claims = {}
end

function ZomboidSeasons.SupplyDrops.saveModData()
    local modData = ModData.getOrCreate("ZomboidSeasons_Drops")
    modData.activeDrops = {}
    for dropId, data in pairs(activeDrops) do
        modData.activeDrops[dropId] = {
            x = data.x, y = data.y, z = data.z,
            poi_name = data.poi_name
        }
    end
    ModData.transmit("ZomboidSeasons_Drops")
end
