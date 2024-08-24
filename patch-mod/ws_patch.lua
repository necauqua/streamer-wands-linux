local function send(data)
    -- skip pings, we have our own in the rust thing
    if data == 'im alive' then
        return
    end
    local f = io.open('streamer-wands.json', 'w')
    if f then
        f:write(data)
        f:close()
    end
end

-- everything except _ws_main is local, so we do the upvalue dance
-- to get the socket and then we monkeypatch it to do our thing

local idx = 1
while true do
    ---@diagnostic disable-next-line: undefined-global
    local name, value = debug.getupvalue(_ws_main, idx)
    if not name then
        break
    end
    if name == 'main_socket' then
        value.status = function() return "open" end
        value.send = function(_, data) send(data) end
        value.poll = function() return true end

        -- nobody in the noita wiki examples calls break when
        -- linearly searching for something, wtf is up with that lol
        break
    end
    idx = idx + 1
end
