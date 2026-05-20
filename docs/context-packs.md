# Context packs

A context pack is a short local briefing for Cursor, Codex, Claude, Continue, VS Code, or a generic AI assistant.

```bash
memory context write --for cursor --output .memory.cpp/context/cursor.md
```

Expected output:

```text
wrote .memory.cpp/context/cursor.md
next: memory map why "MCP integration"
```

What just happened: memory.cpp gathered repo summary, branch state, important files, commands, TODOs, recent memories, and safety notes into one file.

Privacy note: the context file stays local until you paste it or attach it yourself.