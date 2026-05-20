# memory.cpp with claude

Claude Desktop can use the generated MCP server snippet. This repo writes `.claude/claude_desktop_config.json` as a local project-safe template.

`ash
memory attach claude --dry-run
memory attach --print-config claude
memory attach claude --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach claude --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.