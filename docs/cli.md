# CLI Reference

`memory.cpp` ships a small core command tree plus a few launch-polish commands routed through a documented pre-parser.

## Core commands

```bash
memory init
memory remember
memory recall
memory explain
memory forget
memory patch
memory context
memory compile
memory import
memory watch
memory sleep
memory compact
memory timeline
memory replay
memory graph
memory stats
memory list
memory export
memory workspace
memory policy
memory snapshot
memory diff
memory inbox
memory attach
memory serve
memory dashboard
memory proxy
memory mcp
```

## Launch-polish commands

These are currently routed through a small pre-parser to avoid a Clap stack-overflow edge case from an oversized nested command tree.

```bash
memory edit
memory restore
memory demo
memory audit-log
memory doctor
memory dev
memory map
memory start
memory stop
memory status
```

This is intentional for v0.2.1. The behavior is tested and the limitation is documented. A future cleanup can flatten the command tree further.

## Most important workflows

### Remember and recall

```bash
memory --db .memory.cpp/memory.db remember "Use SQLite for local-first durability." --workspace demo --kind decision
memory --db .memory.cpp/memory.db recall "why SQLite" --workspace demo --content
memory --db .memory.cpp/memory.db explain "why SQLite" --workspace demo
```

### Demo seed

```bash
memory --db .memory.cpp/memory.db demo seed --workspace demo --path .
memory --db .memory.cpp/memory.db demo reset --workspace demo
```

### Daily development flow

```bash
memory --db .memory.cpp/memory.db dev watch ./repo --workspace demo
memory --db .memory.cpp/memory.db dev morning --workspace demo
memory --db .memory.cpp/memory.db dev resume "proxy launch" --workspace demo
```

### Maps

```bash
memory --db .memory.cpp/memory.db map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
memory --db .memory.cpp/memory.db map why "MCP integration" --workspace demo --output markdown
memory --db .memory.cpp/memory.db map impact "SQLite storage" --workspace demo --output markdown
memory --db .memory.cpp/memory.db map compare before-launch after-launch --workspace demo --output json
```

### Runtime management

```bash
memory --db .memory.cpp/memory.db start --workspace demo --proxy
memory --db .memory.cpp/memory.db status
memory --db .memory.cpp/memory.db stop
memory --db .memory.cpp/memory.db audit-log --limit 20
```

## Output guidance

- use `--json` for machine-readable CLI output where available
- use `--save <path>` with `memory map` when generating HTML, Mermaid, or Markdown files you want to keep
- use `doctor` before sharing a demo or attaching agents
- create a `.memoryignore` file before using `import` or `dev watch` on a real repository
