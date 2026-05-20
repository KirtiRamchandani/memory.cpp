# Hacker News launch draft

Title: Show HN: memory.cpp - your repo remembers

memory.cpp is a local-first developer tool that helps a repo remember what happened, why it changed, what broke, how it was fixed, and what AI coding tools should know.

The useful first commands are `memory dev morning`, `memory context write --for cursor`, and `memory map --type evolution --output html`.

It is intentionally not a hosted service. The default database is local SQLite under `.memory.cpp/`, MCP is read-only by default, and terminal memory is opt-in.
