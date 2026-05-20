# memory.cpp with vscode

VS Code compatible tools can read `.vscode/mcp.json`. Attach keeps memory tools read-only by default.

`ash
memory attach vscode --dry-run
memory attach --print-config vscode
memory attach vscode --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach vscode --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.