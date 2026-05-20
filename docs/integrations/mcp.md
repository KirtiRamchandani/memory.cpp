# memory.cpp with mcp

MCP exposes read-only memory_search, memory_context, memory_map, memory_timeline, and memory_explain by default. memory_add, memory_patch, and memory_forget require explicit write approval.

`ash
memory attach mcp --dry-run
memory attach --print-config mcp
memory attach mcp --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach mcp --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.