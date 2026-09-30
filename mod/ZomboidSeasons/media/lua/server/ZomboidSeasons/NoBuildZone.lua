-- ZomboidSeasons no-build zone enforcement
-- Prevents player construction within a radius of supply drop POI locations.
-- Uses the same POI coordinates defined in config.prod.toml / SupplyDrops.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.NoBuildZone = {}

local NO_BUILD_RADIUS = 25  -- tiles
local protectedZones = {}   -- { {x=, y=, name=}, ... }

-- Check if a tile position is within any protected zone
local function isProtected(tileX, tileY)
    for _, zone in ipairs(protectedZones) do
        local dx = tileX - zone.x
        local dy = tileY - zone.y
        if dx * dx + dy * dy <= NO_BUILD_RADIUS * NO_BUILD_RADIUS then
            return true, zone.name
        end
    end
    return false, nil
end

-- Hook: prevent building near POIs
-- PZ fires OnDoTileBuilding3(character, x, y, z, sprite)
-- Return false from ISBuildMenu hooks or use Events approach.
-- In B42, we hook Events.OnDoTileBuilding2 which fires (dragObject, bRender, x, y, z, sq)
-- We can also use a LuaEvent check before placement.

local function onObjectAboutToBeAdded(object)
    if not object then return true end

    local sq = object:getSquare()
    if not sq then return true end

    local tileX = sq:getX()
    local tileY = sq:getY()

    local blocked, zoneName = isProtected(tileX, tileY)
    if blocked then
        -- Find nearest player to notify
        local players = getOnlinePlayers()
        if players then
            for i = 0, players:size() - 1 do
                local p = players:get(i)
                local px = p:getX()
                local py = p:getY()
                local dist = math.abs(px - tileX) + math.abs(py - tileY)
                if dist < 30 then
                    p:Say("[ZS] Cannot build near " .. (zoneName or "supply drop zone"))
                    break
                end
            end
        end
        return false
    end
    return true
end

-- Alternative approach: hook into ISBuildAction (more reliable for B42)
local origPerform = nil

local function hookBuildAction()
    if ISBuildAction and ISBuildAction.perform and not origPerform then
        origPerform = ISBuildAction.perform
        ISBuildAction.perform = function(self)
            local chr = self.character
            if chr then
                local sq = self.square or (self.item and self.item:getSquare())
                if sq then
                    local blocked, zoneName = isProtected(sq:getX(), sq:getY())
                    if blocked then
                        chr:Say("[ZS] Cannot build near " .. (zoneName or "supply drop zone"))
                        -- Stop the action
                        ISBaseTimedAction.perform(self)
                        return
                    end
                end
            end
            origPerform(self)
        end
        print("[ZomboidSeasons] NoBuildZone: Hooked ISBuildAction.perform")
    end
end

function ZomboidSeasons.NoBuildZone.init()
    -- Load POI locations from the SupplyDrops module if available
    -- The backend writes POIs to config, but we can also read them from
    -- the SupplyDrops pending data or hardcode known locations.
    -- For now, we'll import from a shared POI list.
    protectedZones = {}

    -- Get POIs from config - the backend writes supply_drops.pois to config.json
    -- Since PZ Lua can't parse JSON easily, we hardcode the known POIs
    -- and they'll be updated when we expand the config parser.
    local pois = ZomboidSeasons.NoBuildZone.getPOIs()
    for _, poi in ipairs(pois) do
        table.insert(protectedZones, poi)
    end

    -- Hook build action
    hookBuildAction()

    -- Also try hooking via Events if available in this PZ version
    if Events.OnObjectAboutToBeAdded then
        Events.OnObjectAboutToBeAdded.Remove(onObjectAboutToBeAdded)
        Events.OnObjectAboutToBeAdded.Add(onObjectAboutToBeAdded)
    end

    print("[ZomboidSeasons] NoBuildZone initialized (" .. #protectedZones .. " protected zones, radius=" .. NO_BUILD_RADIUS .. ")")
end

-- POI definitions - kept in sync with config.prod.toml supply_drops.pois
-- Updated by deploy process; coordinates match the backend config.
function ZomboidSeasons.NoBuildZone.getPOIs()
    return {
        { name = "Bright Flag Inn", x = 8022, y = 11441 },
        { name = "West Point Hotel", x = 12016, y = 6920 },
        { name = "McCoy Logging Corp", x = 10324, y = 9594 },
        { name = "Knox Bank West Point", x = 11909, y = 6913 },
        { name = "West Point Town Hall", x = 11952, y = 6872 },
        { name = "Riverside Pharmacy", x = 6469, y = 5265 },
        { name = "Muldraugh Gas Station", x = 10668, y = 10625 },
        { name = "Muldraugh Police Station", x = 10636, y = 10411 },
        { name = "Muldraugh Warehouse", x = 10617, y = 9313 },
        { name = "Rosewood Fire Station", x = 8148, y = 11730 },
        { name = "Rosewood Police Station", x = 8083, y = 11733 },
        { name = "Rosewood School", x = 8383, y = 11605 },
    }
end
