# Developer Workflow

The `memory dev` namespace is meant to make the repository feel alive.

## `memory dev watch`

```bash
memory --db .memory.cpp/memory.db dev watch ./repo --workspace demo --once
```

This wraps repo-friendly watch behavior and stores imported code/doc chunks as project memory.

## `memory dev morning`

```bash
memory --db .memory.cpp/memory.db dev morning --workspace demo
```

It summarizes:

- yesterday's major changes
- recent decisions
- recent bugs and fixes
- open conflicts
- inbox items
- suggested next work

This is one of the best proof points for the product.

## `memory dev resume`

```bash
memory --db .memory.cpp/memory.db dev resume "MCP integration" --workspace demo
```

It reconstructs a recent workflow around the topic and produces a model-ready context block.

## `memory doctor`

`doctor` belongs next to the dev workflow because it answers the question: "is this environment launch-ready?"

```bash
memory --db .memory.cpp/memory.db doctor --workspace demo
```
