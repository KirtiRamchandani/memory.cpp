# memory.cpp

`memory.cpp` is the missing local memory layer for AI apps.

Think **SQLite for engineering memory**:

- one local file
- fast enough for daily use
- private by default
- explainable instead of magical
- attachable to coding agents and local model runtimes
- visual enough to prove that it works

It is **not finished** and it should not be presented as a mythical final product yet.

What exists today is a strong, launchable **v0.2.1 core**:

- local memory
- repo memory
- visual maps
- developer workflow recap/resume
- MCP and proxy surfaces
- safety defaults for agent integrations

That is the current release story.

## 60-second demo

```bash
memory --db .memory.cpp/memory.db init --workspace demo
memory --db .memory.cpp/memory.db demo seed --workspace demo --path .
memory --db .memory.cpp/memory.db map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
```

Open the generated HTML file.

That is the first "wait, my project can explain itself" moment.

## Why this project exists

Most tools in this space are one of these:

- memory frameworks
- vector databases
- agent platforms

`memory.cpp` is aiming at a different category:

```text
memory.cpp
SQLite for engineering memory.
One file. Local. Fast. Private. Attaches to everything.
```

The long-term direction is broader than a memory database. The project is trying to become a local memory layer for:

- developers
- AI coding agents
- project onboarding
- debugging
- release preparation
- CI and domain packs later

## What is working in v0.2.1

### Core memory engine

- local SQLite-backed durable storage
- global and workspace-aware recall
- importance and confidence persistence
- derived freshness, usefulness, trust, sensitivity, and source-reliability scores
- immutable version history for create/edit/patch/forget/restore
- patch/supersede flow instead of append-only duplication
- review inbox for uncertain or sensitive candidate memory
- `.memoryignore` and `.gitignore` respected during import/watch

### Visual maps

- `memory map --type evolution`
- `memory map --type timeline`
- `memory map --type decisions`
- `memory map --type architecture`
- `memory map --type bugs`
- `memory map --type dependencies`
- outputs: `json`, `markdown`, `mermaid`, `html`
- convenience commands: `memory map why ...`, `memory map impact ...`, `memory map compare ...`

### Developer workflow helpers

- `memory dev watch`
- `memory dev morning`
- `memory dev resume`
- `memory doctor`
- `memory demo`

### AI integration surfaces

- OpenAI-compatible proxy
- MCP stdio server
- attach helpers for Cursor, Codex, Claude, VS Code, and Ollama
- read-only MCP by default
- workspace-scoped MCP access
- agent audit log
- candidate-memory tool for safer unattended agent behavior
- `memory audit-log` for local agent-access receipts

## Quickstart

See the full guide in [docs/quickstart.md](docs/quickstart.md).

The short version:

```bash
memory --db .memory.cpp/memory.db init --workspace demo
memory --db .memory.cpp/memory.db demo seed --workspace demo --path .
memory --db .memory.cpp/memory.db dev morning --workspace demo
memory --db .memory.cpp/memory.db map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
memory --db .memory.cpp/memory.db doctor --workspace demo
memory --db .memory.cpp/memory.db audit-log --limit 10
```

## The best launch surfaces

### `memory map --type evolution`

```bash
memory --db .memory.cpp/memory.db map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
```

This generates a shareable project evolution map.

### `memory dev morning`

```bash
memory --db .memory.cpp/memory.db dev morning --workspace demo
```

This gives a repo recap with recent changes, decisions, bug/fix memory, conflicts, and a suggested next action.

### `memory attach cursor`

```bash
memory --db .memory.cpp/memory.db attach cursor --workspace demo
```

This writes a local MCP config for Cursor with safe defaults.

### `memory proxy`

```bash
memory --db .memory.cpp/memory.db proxy --listen 127.0.0.1:7332 --upstream http://127.0.0.1:11434 --workspace demo
```

Point any OpenAI-compatible client at `http://127.0.0.1:7332/v1`.

## MCP safety defaults

By default, `memory mcp` is intentionally conservative:

- read-only by default
- workspace-scoped access
- no shell execution
- secret redaction on responses
- audit log at `.memory.cpp/audit/mcp-access.jsonl`
- `memory_add_candidate` exposed before direct write tools
- `.memoryignore` patterns respected by import and watch flows

Enable direct write tools only if you really want them:

```bash
memory --db .memory.cpp/memory.db mcp --workspace demo --allow-writes
```

## Install

### Local install from source

```bash
./scripts/install.sh
```

PowerShell:

```powershell
./scripts/install.ps1
```

### Verify and smoke-test

```bash
./scripts/verify.ps1
./scripts/smoke.sh
```

PowerShell:

```powershell
./scripts/smoke.ps1
```

## Documentation

- [Quickstart](docs/quickstart.md)
- [CLI Reference](docs/cli.md)
- [Maps](docs/maps.md)
- [MCP](docs/mcp.md)
- [Proxy](docs/proxy.md)
- [Developer Workflow](docs/dev-workflow.md)
- [Safety](docs/safety.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Map Examples](docs/examples/README.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)

## Project layout

```text
crates/
  memory-core/   engine, storage, ranking, graph, import, maps
  memory-cli/    CLI, MCP, proxy, dashboard, attach helpers
  memory-capi/   stable C ABI
docs/            quickstart, maps, MCP, safety, ADRs, roadmap
include/         public C header
scripts/         install, verify, smoke
evals/           example eval inputs
```

## Current caveat

A small group of launch-polish commands currently use a documented pre-parser because an oversized nested Clap command tree hit a stack-overflow edge case.

That is acceptable for v0.2.1 because:

- the behavior is intentional
- the parser has dedicated tests
- the limitation is documented
- the user-facing commands work

It is still a cleanup target for a future release.

## What comes next

The next useful expansions should happen in layers, not as a giant feature dump:

1. v0.3: automatic candidate memory, Git-aware extraction, stronger local semantic embeddings
2. v0.4: CI and defensive security/audit domain packs
3. v0.5: mobile and webapp domain packs
4. v1.0: sync, team memory, SDKs, deeper governance

That staged path matters because the winning product is not "a memory database."

It is a project that helps engineers and AI agents remember what happened, explain why it changed, and resume work without losing context.
