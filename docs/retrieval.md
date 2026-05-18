# Retrieval

Retrieval is what turns stored memory into a useful engineering assistant instead of a passive note dump.

## Modes

Current launchable retrieval surfaces include:

- `memory recall`
- `memory explain`
- `memory context`
- `memory compile`
- `memory dev resume`
- `memory map`
- MCP `memory_search`, `memory_context`, `memory_map`, `memory_timeline`, `memory_graph`

## Ranking model

The engine combines:

- text similarity
- keyword overlap
- entity overlap
- importance
- confidence
- freshness and trust-derived scores
- sensitivity penalties

The output is explainable: `memory explain` shows why specific memories were surfaced.

## Workspace behavior

Retrieval is workspace-aware, with support for global memory inclusion so repeated project rules can stay available across sessions.

## Token budget

`memory context` and `memory compile` shape recalled memory into prompt-ready blocks under a configurable token budget.

## Good usage pattern

```bash
memory --db .memory.cpp/memory.db recall "why did we choose SQLite" --workspace demo --content
memory --db .memory.cpp/memory.db explain "why did we choose SQLite" --workspace demo
memory --db .memory.cpp/memory.db compile "continue the proxy launch work" --workspace demo --target codex
```
