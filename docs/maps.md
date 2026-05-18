# Maps

`memory map` is the visual proof that memory.cpp is working.

The goal is simple:

Your project should be able to explain how it evolved, why decisions happened, which bugs changed the architecture, and what probably comes next.

## Core commands

```bash
memory map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
memory map . --workspace demo --type timeline --output markdown
memory map . --workspace demo --type decisions --why --output markdown
memory map . --workspace demo --type architecture --output mermaid
memory map . --workspace demo --type bugs --output markdown
memory map . --workspace demo --type dependencies --output mermaid
```

## Shortcut commands

```bash
memory map why "MCP integration" --workspace demo --output markdown
memory map impact "SQLite storage" --workspace demo --output markdown
memory map compare demo-foundation demo-launch-core --workspace demo --output json
```

## Supported v0.2.1 map types

- `evolution`
- `timeline`
- `decisions`
- `architecture`
- `bugs`
- `dependencies`

## Supported outputs

- `json`
- `markdown`
- `mermaid`
- `html`

## Chronological mode

Use `--chronological` when you want the map to emphasize sequence over structure.

```bash
memory map . --workspace demo --type evolution --chronological --output markdown
```

This is useful for demos, onboarding, and release notes.

## Why mode

Use `--why` when you want decisions and milestones to surface reasons and citations.

```bash
memory map . --workspace demo --type decisions --why --output markdown
```

`why` is especially useful for:

- architecture decisions
- launch tradeoffs
- major migrations
- bug/fix history

## Save files directly

`memory map` can write output directly to disk.

```bash
memory map . --workspace demo --type evolution --output html --save .memory.cpp/demo/evolution.html
```

That is the best way to create a shareable local demo artifact.

## Data sources

Map generation prefers:

1. memory events
2. stored memories
3. entity graph / relations
4. docs and README content
5. git history, if available

Git enrichment is optional. The command still works without commits.

## What is intentionally deferred

The following are part of the larger product universe, but not the v0.2.1 launch core:

- `memory map replay`
- fuzzing maps
- audit maps
- webapp maps
- mobile maps
- SVG/PNG output
- Excalidraw / PlantUML / Graphviz exporters

Those are good next expansions after the launchable core feels excellent.
