# ADR 0002: MCP Read-Only By Default

## Status
Accepted

## Decision
Expose MCP in read-only mode by default and require explicit opt-in for direct write tools.

## Why
- AI-tool integration is a launch feature, so safety needs to be visible from day one
- read-only defaults reduce accidental mutation risk
- workspace scoping and audit logging improve trust
- candidate-memory submission is safer than silent durable writes

## Consequences
- `memory_search`, `memory_context`, `memory_explain`, `memory_graph`, and `memory_map` are the default MCP surface
- `memory_add_candidate` is preferred over direct mutation for unattended agents
