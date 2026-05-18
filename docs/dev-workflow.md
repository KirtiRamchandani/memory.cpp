# Developer Workflow

The `memory dev` namespace is meant to make the repository feel alive.

## `memory dev watch`

```bash
memory --db .memory.cpp/memory.db dev watch ./repo --workspace demo --once
```

This wraps repo-friendly watch behavior and stores imported code/doc chunks as project memory.

`dev watch` respects `.memoryignore` and `.gitignore`, so secrets, build output, and bulky vendor folders can stay out of memory by default.

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

## `memory dev explain-repo`

```bash
memory --db .memory.cpp/memory.db dev explain-repo . --workspace demo
```

This gives a compact onboarding summary:

- repo root
- top-level project shape
- recent decisions
- recent bugs and fixes
- recent git activity when available

## `memory dev next`

```bash
memory --db .memory.cpp/memory.db dev next --workspace demo
```

This is the forward-looking companion to `dev morning`. It prioritizes:

- pending inbox review
- conflicts
- current task/decision threads
- bug/fix follow-up
- git refresh suggestions

## `memory doctor`

`doctor` belongs next to the dev workflow because it answers the question: "is this environment launch-ready?"

```bash
memory --db .memory.cpp/memory.db doctor --workspace demo
```

## `memory audit-log`

When you attach an agent through MCP, you can inspect local usage receipts:

```bash
memory --db .memory.cpp/memory.db audit-log --limit 20
```

## Git-aware developer memory

```bash
memory --db .memory.cpp/memory.db git summary --since 14d
memory --db .memory.cpp/memory.db git ingest --workspace demo --since 14d
memory --db .memory.cpp/memory.db extract . --workspace demo --dry-run
```

This makes the developer workflow feel less manual:

- `git summary` shows the recent project arc
- `git ingest` turns commit history into memory candidates or direct stored memories when confidence is high
- `extract` turns docs, comments, and workflow notes into reviewable candidate memory
