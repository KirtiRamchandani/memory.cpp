# Stateful Memory

`memory.cpp` can model lightweight local state for people, agents, projects, tools, work sessions, and derived insights.

It does this without a cloud account or a new service. Entities, sessions, events, and insights are stored as ordinary local memories with structured metadata, so recall, timelines, context packs, maps, and reports can reuse them immediately.

## Create Entities

```bash
memory entity create --type agent --name Codex
memory entity create --type project --name memory.cpp
memory entity link Codex memory.cpp --relation works_on
memory entity list
```

Expected output shape:

```text
entity stored: <memory-id>
name: Codex
type: agent

entity relation stored: <memory-id>
```

What happened: memory.cpp stored the agent, project, and relation locally as evidence-backed memories. Nothing was uploaded.

## Record A Session

```bash
memory session start --name release-polish --goal "finish launch readiness"
memory session add-message --role user --text "Need README and smoke checks finished."
memory session add-event --type test --text "cargo test passed"
memory session summarize
```

Expected output shape:

```text
session started: <memory-id>
session event stored: <memory-id>
Session summary for <session-id>
- Session started: release-polish...
- Session test: cargo test passed
```

What happened: memory.cpp created a local work-session trail that can be summarized, searched, and included in AI context packs.

## Derive Local Insights

```bash
memory insight derive --scope repo
memory insight list --scope repo
memory insight show <insight-id>
```

Expected output shape:

```text
insight stored: <memory-id>
evidence memories: [...]
```

What happened: memory.cpp inspected recent local memories in the scope and wrote a deterministic project insight with evidence memory IDs.

## Privacy Notes

- All state is local by default.
- Insights cite local memory IDs instead of inventing facts.
- Use `memory memories show <id>` or `memory evidence <id>` to inspect source metadata.
- Use `memory forget <id>` or `memory resolve <id> --stale` when state is no longer useful.
