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
        break
    end
    idx = idx + 1
end
