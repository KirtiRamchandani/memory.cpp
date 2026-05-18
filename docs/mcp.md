# MCP

`memory.cpp` uses MCP as the default integration surface for coding agents.

## Launch default: conservative and local

By default, `memory mcp` is designed to be safe for local developer use:

- read-only by default
- workspace-scoped access
- secret redaction before tool responses
- audit log for agent access
- no shell execution support

## Start MCP safely

```bash
memory --db .memory.cpp/memory.db mcp --workspace demo
```

This keeps the agent scoped to `demo` and only exposes safe read tools plus candidate-memory submission.

## Default tools

Read tools available by default:

- `memory_search`
- `memory_context`
- `memory_timeline`
- `memory_explain`
- `memory_graph`
- `memory_map`
- `memory_add_candidate`

`memory_add_candidate` does not directly mutate durable memory. It queues candidate memory for review when appropriate.

## Write tools

Direct write tools are disabled by default.

To enable them deliberately:

```bash
memory --db .memory.cpp/memory.db mcp --workspace demo --allow-writes
```

This exposes:

- `memory_add`
- `memory_update`
- `memory_forget`
- `memory_compact`

Use this mode carefully.

## Attach helpers

```bash
memory --db .memory.cpp/memory.db attach cursor --workspace demo
memory --db .memory.cpp/memory.db attach codex --workspace demo
memory --db .memory.cpp/memory.db attach claude --workspace demo
memory --db .memory.cpp/memory.db attach vscode --workspace demo
```

The generated MCP config points at `memory.cpp` in read-only mode by default and preserves workspace scoping.

## Audit log

Agent access is logged to:

```text
.memory.cpp/audit/mcp-access.jsonl
```

This gives you a local trail of which tools were called and whether they were allowed.
