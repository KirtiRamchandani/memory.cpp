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

## Compile a tighter task pack

When the assistant only needs one task, use the context compiler:

```bash
memory compile "fix checkout bug" --provider openai --budget 1500
memory token-firewall "fix checkout bug"
memory cache-plan "fix checkout bug" --provider claude
memory kv-report "fix checkout bug"
```

What just happened: memory.cpp filtered local repo memory, removed stale or duplicated context, kept hard rules, and printed an estimated token/KV pressure report. It does not directly compress a provider KV cache; it sends less unnecessary context.

See [context compiler and token firewall](context-compiler.md).
