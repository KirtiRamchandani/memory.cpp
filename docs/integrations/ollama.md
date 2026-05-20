# memory.cpp with ollama

Ollama integration uses proxy instructions, not automatic background services. Use `memory attach ollama --dry-run`, then start `memory proxy` explicitly if wanted.

`ash
memory attach ollama --dry-run
memory attach --print-config ollama
memory attach ollama --yes
memory attach status
`

What just happened: memory.cpp printed or wrote a local config snippet and kept MCP read-only by default.

Undo:

`ash
memory detach ollama --dry-run
`

Privacy note: nothing is uploaded; the config points at your local memory command.