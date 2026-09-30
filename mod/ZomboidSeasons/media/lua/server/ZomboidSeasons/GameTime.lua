-- ZomboidSeasons game-time reporter
-- Writes current in-game date/time to gametime.json every game minute
-- so the backend can expose it via API for the website.

ZomboidSeasons = ZomboidSeasons or {}
ZomboidSeasons.GameTime = {}

local dataDir = nil

local function writeGameTime()
    if not dataDir then return end

    local gt = getGameTime()
    if not gt then return end

    local month = gt:getMonth() + 1  -- PZ months are 0-indexed
    local day = gt:getDay() + 1      -- PZ days are 0-indexed
    local hour = gt:getTimeOfDay()    -- float 0.0-24.0
    local hourInt = math.floor(hour)
    local minute = math.floor((hour - hourInt) * 60)

    local worldAge = gt:getWorldAgeHours()

    local json = string.format(
        '{"month":%d,"day":%d,"hour":%d,"minute":%d,"world_age_hours":%.2f}',
        month, day, hourInt, minute, worldAge
    )

    local path = dataDir .. "/gametime.json"
    local writer = getFileWriter(path, true, false)
    if writer then
        writer:write(json)
        writer:close()
    end
end

function ZomboidSeasons.GameTime.init()
    local dir = ZomboidSeasons.Config and ZomboidSeasons.Config.get("data_dir")
    dataDir = dir or "ZomboidSeasons"

    Events.EveryOneMinute.Remove(writeGameTime)
    Events.EveryOneMinute.Add(writeGameTime)

    -- Write immediately on init
    writeGameTime()

    print("[ZomboidSeasons] GameTime initialized (writing to " .. dataDir .. "/gametime.json)")
end
