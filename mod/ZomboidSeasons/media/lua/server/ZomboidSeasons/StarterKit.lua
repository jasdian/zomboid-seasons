-- ZomboidSeasons starter kit module
-- Gives every player a school bag, kitchen knife, axe, and first aid kit on spawn.
-- Uses player-level ModData so kit is given once per life (resets on death/respawn).
-- Delays giving items by 1 tick to ensure the player is fully loaded.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.StarterKit = {}

local STARTER_ITEMS = {
    "Base.Bag_Schoolbag",
    "Base.KitchenKnife",
    "Base.HandAxe",
}

-- Items to place inside the First Aid Kit container
local FIRST_AID_CONTENTS = {
    "Base.Bandage",
    "Base.Bandage",
    "Base.AlcoholWipes",
    "Base.AlcoholWipes",
    "Base.Pills",
    "Base.CottonBalls",
}

-- Players waiting for kit delivery (steamId -> {player, username, tick})
local pendingKits = {}

function ZomboidSeasons.StarterKit.init()
    if ZomboidSeasons.StarterKit._evtRef then
        Events.EveryOneMinute.Remove(ZomboidSeasons.StarterKit._evtRef)
    end
    ZomboidSeasons.StarterKit._evtRef = ZomboidSeasons.StarterKit.checkPlayers
    Events.EveryOneMinute.Add(ZomboidSeasons.StarterKit._evtRef)
    pendingKits = {}
    print("[ZomboidSeasons] StarterKit initialized")
end

local function giveKit(player, username)
    local inv = player:getInventory()
    if not inv then
        print("[ZomboidSeasons] StarterKit: WARNING no inventory for " .. username)
        return false
    end

    for _, itemType in ipairs(STARTER_ITEMS) do
        inv:AddItem(itemType)
    end

    -- Add First Aid Kit filled with medical supplies
    local kit = inv:AddItem("Base.FirstAidKit")
    if kit then
        local kitInv = nil
        if kit.getInventory then kitInv = kit:getInventory() end
        if not kitInv and kit.getItemContainer then kitInv = kit:getItemContainer() end
        if kitInv then
            for _, itemType in ipairs(FIRST_AID_CONTENTS) do
                kitInv:AddItem(itemType)
            end
        else
            for _, itemType in ipairs(FIRST_AID_CONTENTS) do
                inv:AddItem(itemType)
            end
        end
    end

    player:getModData().ZS_StarterKit = true
    print("[ZomboidSeasons] Gave starter kit to " .. username)
    return true
end

function ZomboidSeasons.StarterKit.checkPlayers()
    local players = getOnlinePlayers()
    if not players then return end

    -- Phase 1: deliver pending kits (players seen last tick, now fully loaded)
    for steamId, info in pairs(pendingKits) do
        local player = info.player
        if player and not player:isDead() and player:getX() ~= 0 then
            giveKit(player, info.username)
        end
        pendingKits[steamId] = nil
    end

    -- Phase 2: detect new characters, queue for next tick
    for i = 0, players:size() - 1 do
        local player = players:get(i)
        if player and not player:isDead() then
            local pd = player:getModData()
            if not pd.ZS_StarterKit then
                local steamId = ZomboidSeasons.Utils.getSteamId(player)

                -- Already pending? skip
                if not pendingKits[steamId] then
                    local username = player:getUsername() or "unknown"

                    -- Assign character ID and emit event immediately
                    local now = getTimestampMs()
                    local charId = steamId .. "_" .. now
                    pd.ZS_CharId = charId

                    local worldAge = 0
                    local gt = getGameTime()
                    if gt then worldAge = gt:getWorldAgeHours() end

                    ZomboidSeasons.EventTracker.emit(
                        "character_created",
                        steamId,
                        username,
                        { lua_char_id = charId, world_age_hours = worldAge },
                        "cc_" .. steamId .. "_" .. now
                    )
                    print("[ZomboidSeasons] StarterKit: new character " .. username .. " — kit queued for next tick")

                    -- Queue kit delivery for next game-minute tick
                    pendingKits[steamId] = { player = player, username = username }
                end
            end
        end
    end
end
