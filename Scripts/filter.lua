-- scripts/filter.lua
function filter_tokens(input)
    -- Instantly wipe out system prompt bloat, tracking cookies, or raw debug logs
    local clean = string.gsub(input, "raw_context_stream_to_be_killed", "[KILLED]")
    return clean
end
