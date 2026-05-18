# Memory Lifecycle

`memory.cpp` keeps the storage model intentionally small and durable.

## Core objects

A memory has:

- kind
- workspace scope
- tags
- metadata
- provenance
- importance/confidence
- immutable version history

The project avoids creating a separate schema per domain pack. CI, proxy, agent, repo, and map flows all build on the same core memory model.

## Lifecycle stages

### Create

`remember`, `import`, demo seed, proxy learning, extraction, and MCP candidate tools all create memory or candidate memory.

### Review

Lower-confidence or policy-gated writes go into the inbox for human review.

### Recall

Search, context compilation, explain, timeline, graph, and map generation all retrieve memory through the same engine.

### Edit / patch

`memory edit` and `memory patch` preserve history instead of mutating silently.

### Forget / restore

`memory forget` performs a soft-delete style transition.
`memory restore` brings the latest active version back.

### Snapshot / diff

Snapshots let you compare memory state over time and feed map comparisons.

## Policies and safety

Policies can:

- force manual review
- reject certain memory kinds
- set retention or decay behavior
- bias candidate approval

Sensitive content is blocked or redacted before it becomes durable memory.
