# memory.cpp with continue

Continue can use `.continue/mcp.json` as a local MCP snippet. Write-capable memory tools stay disabled by default.

`ash
memory attach continue --dry-run
memory attach --print-config continue
memory attach continue --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach continue --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.