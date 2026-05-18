# Architecture

`memory.cpp` is designed as a local memory primitive, not just a retrieval helper.

```text
input
  -> normalize and compress
  -> score for sensitivity and confidence
  -> embed
  -> extract entities
  -> persist in SQLite
  -> record timeline events
  -> recall with hybrid ranking
  -> return memories, context blocks, graph views, or proxy injections
```

## Crates

`memory-core`

- storage schema
- embedding abstraction
- ranking engine
- memory lifecycle
- contradiction tracking
- timeline, graph, persona, policy, snapshot, and inbox logic

`memory-cli`

- human-facing CLI
- local API server
- MCP stdio server
- OpenAI-compatible memory proxy
- app attach helpers

`memory-capi`

- stable C ABI for embeddings hosts, desktop apps, plugins, and language bindings

## Storage Model

SQLite is the source of truth.

Key tables:

- `memories`
- `memory_entities`
- `memory_events`
- `memory_relations`
- `memory_versions`
- `workspaces`
- `policies`
- `snapshots`
- `memory_conflicts`
- `memory_inbox`

Every memory carries:

- id
- scope
- kind
- content
- summary
- metadata JSON
- importance
- timestamps
- access counters
- embedding bytes
- derived attributes such as confidence, status, permission, and layer

## Retrieval Model

Recall is intentionally hybrid.

The ranker combines:

- semantic similarity
- keyword overlap
- entity relevance
- importance
- recency
- confidence
- redundancy penalty
- sensitivity penalty

This keeps `memory.cpp` from becoming "just another vector store".

## Memory Lifecycle

The project treats memory as something that evolves:

- `remember` stores durable facts
- `patch` supersedes stale memory instead of duplicating it
- `edit` and `restore` preserve immutable version history
- `sleep` compacts, decays, and detects contradictions
- `timeline` exposes the history of change
- `snapshot` captures reversible memory states
- `policy` controls retention behavior
- `inbox` catches uncertain candidates for review

## Product Surfaces

The same engine powers multiple integration modes:

- `memory serve` for local JSON APIs and dashboard access
- `memory proxy` for OpenAI-compatible chat interception
- `memory mcp` for MCP-aware clients
- `memory attach` for project-local configuration bootstrapping
- `memory map` for evolution, decision, architecture, bug, and dependency views
- `memory dev` for morning recap, repo watching, and task resumption
- `memory start|stop|status` as a light runtime wrapper around server/proxy modes
- `memory import` and `memory watch` for ingestion
- `memory export` for portability

## Default Design Choices

- SQLite first, no external database
- offline deterministic embeddings by default
- small dependency set
- local-first workflows
- explainable recall
- explicit workspace boundaries
- embeddable core before hosted features

## What Is Not Done Yet

These are intentionally left for later instead of faked:

- encrypted-at-rest store format
- cloud sync
- Python and Node SDKs
- plugin marketplace
- full team memory permissions
- large-scale ANN indexing
