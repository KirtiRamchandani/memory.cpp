# Use memory.cpp with Claude

Goal: use memory.cpp with claude.

`ash
memory dev context --for claude
memory attach claude --dry-run
memory attach --print-config claude
`

Expected output: a short status, report, or generated file path with an exact next command.

What happened: Claude receives a read-only local MCP snippet or a pasteable context pack.

Privacy note: data stays local unless you copy or attach it yourself.

Next step: run memory doctor if setup feels off.