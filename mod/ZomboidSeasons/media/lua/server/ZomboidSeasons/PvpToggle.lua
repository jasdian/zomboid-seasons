-- ZomboidSeasons PVP opt-in module
-- PVP is opt-in: safety is enabled by default on spawn.
-- Players use PZ's built-in safety toggle (bottom-right heart icon) to opt into PVP.
-- Requires SafetySystem=true and PVPMelee=true in server .ini.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.PvpToggle = {}

-- Enable safety (PVP protection) when a player first spawns
local function onPlayerCreated(player)
    if not player then return end
    local safety = player:getSafety()
    if safety then
        safety:setEnabled(true)
        safety:setCooldown(0)
        print("[ZomboidSeasons] PvpToggle: safety enabled for " .. (player:getUsername() or "?"))
    end
end

function ZomboidSeasons.PvpToggle.init()
    Events.OnCreatePlayer.Remove(onPlayerCreated)
    Events.OnCreatePlayer.Add(onPlayerCreated)
    print("[ZomboidSeasons] PvpToggle initialized (PVP opt-in, safety ON by default)")
end
