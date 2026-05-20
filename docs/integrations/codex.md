# memory.cpp with codex

Codex works best with `memory dev context --for codex` or `memory context write --for codex`. Direct attach writes a local `.codex/mcp.json` snippet when requested.

`ash
memory attach codex --dry-run
memory attach --print-config codex
memory attach codex --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach codex --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.