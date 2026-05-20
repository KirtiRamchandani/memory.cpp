# Examples

`memory.cpp` examples are intentionally short. Each one shows the command, the shape of the output, and what to try next.

## Daily recap

```bash
memory dev morning
```

Output shape:

```text
What you were doing
- Parser cleanup and developer-ready docs.

What changed recently
- Branch: main
- Uncommitted files: 3

What to do next
- Run: memory dev next
```

## Why did this exist?

```bash
memory map why "SQLite storage"
```

Output shape:

```text
Why SQLite storage exists
- Local-first database that travels with the repo.
- Supports provenance, workspaces, candidate review, and maps.

Try next
- memory map impact "SQLite storage"
```

## What should Codex know?

```bash
memory dev context --for codex
```

Output shape:

```text
Project context for Codex
- Summary
- Recent decisions
- Important files
- Commands to run
- Known pitfalls
- Privacy note
```

## What broke before?

```bash
memory dev recall-error "ECONNRESET"
```

Output shape:

```text
Previous fixes
- Restart the local test database.
- Wait for the service port to be free.
- Re-run the focused test command.
```

## More examples

The `examples/` directory contains concise static examples for:

- `memory dev morning`
- `memory dev next`
- `memory dev context --for cursor`
- `memory privacy status`
- `memory ci explain-failure`
- `memory git summary`
- HTML and Markdown project maps
