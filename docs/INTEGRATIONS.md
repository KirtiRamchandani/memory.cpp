# Integrations

## Ollama Proxy

Start Ollama, then place `memory.cpp` in front of it:

```bash
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace user
```

Point any OpenAI-compatible client at:

```text
http://127.0.0.1:7332/v1
```

Flow:

1. Client sends `POST /v1/chat/completions`
2. `memory.cpp` extracts the latest user query
3. relevant long-term memory is recalled
4. memory context is injected into the request
5. request is forwarded upstream
6. response text is scanned for candidate memory
7. low-confidence candidates go to the inbox instead of being silently stored

## Cursor, VS Code, Codex, Claude

Generate project-local MCP config:

```bash
memory --db .memory.cpp/memory.db attach cursor
memory --db .memory.cpp/memory.db attach vscode
memory --db .memory.cpp/memory.db attach codex
memory --db .memory.cpp/memory.db attach claude
```

This writes config files into the current project:

- `.cursor/mcp.json`
- `.vscode/mcp.json`
- `.codex/mcp.json`
- `.claude/claude_desktop_config.json`

Each config points the client at:

```text
memory --db <path> mcp
```

## Ollama Attach Helper

```bash
memory --db .memory.cpp/memory.db attach ollama --host 127.0.0.1 --upstream http://127.0.0.1:11434
```

This writes:

- `.memory.cpp/attach/ollama-proxy.json`

Optional:

```bash
memory --db .memory.cpp/memory.db attach ollama --start-proxy
```

## Local API

```bash
memory --db .memory.cpp/memory.db serve --host 127.0.0.1 --port 7331
```

Key endpoints:

- `GET /health`
- `GET /v1/stats`
- `GET /v1/memories/search?q=...&scope=demo`
- `GET /v1/memories/graph?scope=demo`
- `POST /v1/memories`
- `POST /v1/memories/compact`
- `POST /v1/recall`
- `POST /v1/context`
- `POST /v1/timeline`

Store memory:

```bash
curl -X POST http://127.0.0.1:7331/v1/memories \
  -H "content-type: application/json" \
  -d '{"content":"The user prefers tiny local AI tools.","workspace":"user","kind":"preference","confidence":0.95}'
```

Search memory:

```bash
curl "http://127.0.0.1:7331/v1/memories/search?q=preferred%20tools&scope=user"
```

Build context:

```bash
curl -X POST http://127.0.0.1:7331/v1/context \
  -H "content-type: application/json" \
  -d '{"query":"How should I design the API?","workspace":"user","tokens":800}'
```

Inspect the graph:

```bash
curl "http://127.0.0.1:7331/v1/memories/graph?scope=user&entity=Ollama"
```

## MCP Stdio

Run:

```bash
memory --db .memory.cpp/memory.db mcp
```

Supported protocol methods:

- `initialize`
- `tools/list`
- `tools/call`

Exposed tools:

- `memory_add`
- `memory_search`
- `memory_update`
- `memory_forget`
- `memory_timeline`
- `memory_explain`
- `memory_graph`
- `memory_compact`
