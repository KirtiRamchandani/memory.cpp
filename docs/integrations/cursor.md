# memory.cpp with cursor

Cursor reads MCP config from `.cursor/mcp.json` in this repo. Use `memory attach cursor --dry-run` first, then `memory attach cursor --yes` when ready.

`ash
memory attach cursor --dry-run
memory attach --print-config cursor
memory attach cursor --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach cursor --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.