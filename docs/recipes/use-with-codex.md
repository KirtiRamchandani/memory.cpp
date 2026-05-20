# Use memory.cpp with Codex

Goal: use memory.cpp with codex.

`ash
memory dev context --for codex
memory context write --for codex --output .memory.cpp/context/codex.md
memory attach codex --dry-run
`

Expected output: a short status, report, or generated file path with an exact next command.

What happened: A concise Codex-ready project briefing is generated locally.

Privacy note: data stays local unless you copy or attach it yourself.

Next step: run memory doctor if setup feels off.