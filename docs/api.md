# API

The local HTTP surface is intentionally small and launch-oriented.

## Runtime

```bash
memory --db .memory.cpp/memory.db start --workspace demo
memory --db .memory.cpp/memory.db status
memory --db .memory.cpp/memory.db stop
```

The runtime writes state under `.memory.cpp/runtime/`.

## Endpoints

### `GET /health`

Basic health probe for the server or proxy.

### `GET /v1/map`

Returns a generated map payload from query parameters.

### `POST /v1/map`

Generates a map from a JSON request body.

### `GET /dashboard/map`

Dashboard-oriented map view.

## MCP companion surface

For agent integrations, the more important launch interface is MCP:

- `memory_search`
- `memory_context`
- `memory_map`
- `memory_timeline`
- `memory_graph`
- `memory_add_candidate`

This split keeps the HTTP API small while the agent-facing safety model stays explicit and audited.
