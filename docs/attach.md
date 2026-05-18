# Attach

`memory attach` writes local integration configs so coding tools can consume `memory.cpp` without custom glue.

## Supported targets

```bash
memory --db .memory.cpp/memory.db attach cursor --workspace demo
memory --db .memory.cpp/memory.db attach codex --workspace demo
memory --db .memory.cpp/memory.db attach claude --workspace demo
memory --db .memory.cpp/memory.db attach vscode --workspace demo
memory --db .memory.cpp/memory.db attach ollama --workspace demo --start-proxy
```

## What gets written

- Cursor: `.cursor/mcp.json`
- Codex: `.codex/mcp.json`
- VS Code: `.vscode/mcp.json`
- Claude Desktop: `.claude/claude_desktop_config.json`
- Ollama helper: `.memory.cpp/attach/ollama-proxy.json`

## Safety defaults

Attach helpers intentionally preserve the conservative MCP posture:

- read-only by default
- workspace-scoped access
- secret redaction enabled
- audit logging enabled

That means the first integration experience is useful without handing an agent wide write power.

## Ollama flow

If you use `attach ollama --start-proxy`, the helper starts the local proxy in safe learning mode:

- memory recall is injected before upstream requests
- response-derived memory learning is enabled
- extracted memories are routed through approval-required candidate review
